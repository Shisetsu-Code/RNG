//! Ataque estadistico de horizonte largo y streaming.
//!
//! Entrena predictores solo con el primer tramo de la secuencia, congela
//! esos modelos y evalua el resto sin ver resultados futuros. Mantiene
//! el historial necesario en memoria acotada: O(TYPES) + buffers de lag.
//! No prueba que el CSPRNG sea seguro; detecta patrones estadisticos.

use crate::audit::{Mode, sample_trace};
use crate::{BETS_PER_PACKAGE, Catalog, TYPES};
use rand::Rng;
use serde::Serialize;
use std::time::Instant;

const CLASS_COUNT: usize = 8;
pub const LAGS: [usize; 6] = [1, 2, 8, 32, 100, 1000];
const LONGEST_LAG: usize = 1000;
const THRESHOLD_Z: f64 = 5.0;
const SMOOTHING_1: f64 = 8.0;
const SMOOTHING_2: f64 = 64.0;

fn tier_index(payout: u32) -> usize {
    match payout {
        0 => 0,
        10_000 => 1,
        15_000 => 2,
        20_000 => 3,
        35_200 => 4,
        40_000 => 5,
        80_000 => 6,
        160_000 => 7,
        _ => panic!("premio fuera de los 8 niveles conocidos: {payout}"),
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct IdPredictor {
    pub name: String,
    pub evaluated: u64,
    pub hits: u64,
    pub accuracy_percent: f64,
    pub random_baseline_percent: f64,
    pub expected_random_hits: f64,
    pub excess_hits_z_approx: f64,
    pub statistically_significant_excess: bool,
}

#[derive(Default)]
struct IdAccum {
    hits: u64,
    n: u64,
}

impl IdAccum {
    fn check(&mut self, expected: u32, observed: u32) {
        self.n += 1;
        if expected == observed {
            self.hits += 1;
        }
    }

    fn report(self, name: &str) -> IdPredictor {
        let p = 1.0 / TYPES as f64;
        let expectation = self.n as f64 * p;
        let variance = self.n as f64 * p * (1.0 - p);
        let z = if variance > 0.0 {
            (self.hits as f64 - expectation) / variance.sqrt()
        } else {
            0.0
        };
        IdPredictor {
            name: name.to_owned(),
            evaluated: self.n,
            hits: self.hits,
            accuracy_percent: if self.n == 0 { 0.0 } else {
                100.0 * self.hits as f64 / self.n as f64
            },
            random_baseline_percent: 100.0 * p,
            expected_random_hits: expectation,
            excess_hits_z_approx: z,
            // Approximation unreliable with tiny expected counts; require
            // at least 30 expected random hits to flag any predictor.
            statistically_significant_excess: expectation >= 30.0 && z > THRESHOLD_Z,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct LagReport {
    pub lag: usize,
    pub comparisons: u64,
    pub same_id_hits: u64,
    pub expected_same_id_hits: f64,
    pub same_id_excess_z_approx: f64,
    pub numeric_id_correlation: f64,
    pub numeric_correlation_z_approx: f64,
    pub significant_correlation: bool,
}

#[derive(Default)]
struct LagAccumulator {
    n: u64,
    equal: u64,
    normalized_products: f64,
}

impl LagAccumulator {
    fn sample(&mut self, id: u32, older: u32) {
        let mean = (TYPES as f64 + 1.0) / 2.0;
        let var = (TYPES as f64 * TYPES as f64 - 1.0) / 12.0;
        self.n += 1;
        self.equal += u64::from(id == older);
        self.normalized_products += (id as f64 - mean) * (older as f64 - mean) / var;
    }

    fn report(self, lag: usize) -> LagReport {
        let p = 1.0 / TYPES as f64;
        let expected = self.n as f64 * p;
        let variance = self.n as f64 * p * (1.0 - p);
        let equal_z = if variance > 0.0 {
            (self.equal as f64 - expected) / variance.sqrt()
        } else { 0.0 };
        let corr = if self.n > 0 {
            self.normalized_products / self.n as f64
        } else { 0.0 };
        let corr_z = corr * (self.n as f64).sqrt();
        LagReport {
            lag,
            comparisons: self.n,
            same_id_hits: self.equal,
            expected_same_id_hits: expected,
            same_id_excess_z_approx: equal_z,
            numeric_id_correlation: corr,
            numeric_correlation_z_approx: corr_z,
            significant_correlation: corr_z.abs() > THRESHOLD_Z
                || (expected >= 30.0 && equal_z.abs() > THRESHOLD_Z),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct CategoryPredictor {
    pub name: String,
    pub evaluated: u64,
    pub accuracy_percent: f64,
    pub log_loss_nats_per_result: f64,
    pub baseline_log_loss_nats_per_result: f64,
    pub relative_log_loss_improvement_percent: f64,
    pub improvement_z_approx: f64,
    pub statistically_significant_improvement: bool,
}

#[derive(Default)]
struct CategoryAccumulator {
    n: u64,
    correct: u64,
    sum_logloss: f64,
    baseline_logloss: f64,
    improvement_sum: f64,
    improvement_sq: f64,
}

impl CategoryAccumulator {
    fn observe(
        &mut self,
        class: usize,
        predicted: usize,
        model_prob: f64,
        baseline_prob: f64,
    ) {
        self.n += 1;
        if predicted == class {
            self.correct += 1;
        }
        let model_loss = -model_prob.max(1e-300).ln();
        let base_loss = -baseline_prob.max(1e-300).ln();
        let improvement = base_loss - model_loss;
        self.sum_logloss += model_loss;
        self.baseline_logloss += base_loss;
        self.improvement_sum += improvement;
        self.improvement_sq += improvement * improvement;
    }

    fn report(self, name: &str) -> CategoryPredictor {
        let n = self.n as f64;
        let mean = if n > 0.0 { self.improvement_sum / n } else { 0.0 };
        let variance = if n > 0.0 {
            (self.improvement_sq / n - mean * mean).max(0.0)
        } else { 0.0 };
        let z = if variance > 0.0 {
            mean * n.sqrt() / variance.sqrt()
        } else { 0.0 };
        CategoryPredictor {
            name: name.to_owned(),
            evaluated: self.n,
            accuracy_percent: if n > 0.0 { 100.0 * self.correct as f64 / n } else { 0.0 },
            log_loss_nats_per_result: if n > 0.0 { self.sum_logloss / n } else { 0.0 },
            baseline_log_loss_nats_per_result: if n > 0.0 { self.baseline_logloss / n } else { 0.0 },
            relative_log_loss_improvement_percent: if self.baseline_logloss > 0.0 {
                100.0 * self.improvement_sum / self.baseline_logloss
            } else { 0.0 },
            improvement_z_approx: z,
            statistically_significant_improvement: z > THRESHOLD_Z && mean > 0.0,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct LongAttackReport {
    pub mode: Mode,
    pub observations: u64,
    pub training_observations: u64,
    pub holdout_observations: u64,
    pub training_fraction: f64,
    pub elapsed_seconds: f64,
    pub id_predictors: Vec<IdPredictor>,
    pub payout_class_predictors: Vec<CategoryPredictor>,
    pub lags: Vec<LagReport>,
    pub holdout_type_chi_square_z_approx: Option<f64>,
    pub holdout_type_chi_square_eligible: bool,
    pub first_order_payout_chi_square_z_approx: Option<f64>,
    pub observed_package_boundaries: u64,
    pub boundary_repeat_hits: u64,
    pub expected_boundary_repeat_hits: f64,
    pub any_flagged_statistical_pattern: bool,
    pub warnings: Vec<String>,
}

/// Consume N resultados en una sola secuencia continua, incluso al cruzar
/// limites de los paquetes. Los modelos se entrenan con 70% inicial y
/// se evaluan con el 30% final, sin actualizaciones posteriores.
pub fn observe_long_sequence<R: Rng + ?Sized>(
    catalog: &Catalog,
    mode: Mode,
    observations: u64,
    rng: &mut R,
) -> Result<LongAttackReport, String> {
    crate::validate_catalog(catalog)?;
    if observations < 1000 {
        return Err("--long-bets debe ser al menos 1000".to_owned());
    }
    if observations > usize::MAX as u64 {
        return Err("la secuencia excede el tamaño representable en esta plataforma".to_owned());
    }
    let start = Instant::now();
    let train_end = observations * 7 / 10;
    let test_len = observations - train_end;

    // Los IDs se usan como indices de arrays de 128.000 elementos.
    // O(128000) RAM independientemente de la longitud total.
    let mut train_id_counts = vec![0_u64; TYPES];
    let mut trained_successor = vec![0_u32; TYPES];
    let mut class_counts = [0_u64; CLASS_COUNT];
    let mut first = [[0_u64; CLASS_COUNT]; CLASS_COUNT];
    let mut second = [[[0_u64; CLASS_COUNT]; CLASS_COUNT]; CLASS_COUNT];
    let mut test_id_counts = vec![0_u32; TYPES];

    let mut ring = [0_u32; LONGEST_LAG + 1];
    let mut previous: Option<u32> = None;
    let mut previous_class: Option<usize> = None;
    let mut penultimate_class: Option<usize> = None;
    let mut blind_id = 1_u32;
    let mut blind_count = 0_u64;

    let mut id_blind = IdAccum::default();
    let mut id_increment = IdAccum::default();
    let mut id_repeat = IdAccum::default();
    let mut id_markov = IdAccum::default();
    let mut id_lag100 = IdAccum::default();
    let mut id_lag1000 = IdAccum::default();

    let mut baseline = CategoryAccumulator::default();
    let mut markov1 = CategoryAccumulator::default();
    let mut markov2 = CategoryAccumulator::default();
    let mut lag_reports: [LagAccumulator; LAGS.len()] =
        std::array::from_fn(|_| LagAccumulator::default());
    let mut boundary_count = 0_u64;
    let mut boundary_repeats = 0_u64;

    let mut test_pairs = [[0_u64; CLASS_COUNT]; CLASS_COUNT];

    let mut package_no = 0_u64;
    let mut current_index = 0_u64;
    while current_index < observations {
        let trace = sample_trace(mode, package_no, catalog, rng);
        package_no += 1;
        for (offset, &id) in trace.iter().enumerate() {
            if current_index >= observations {
                break;
            }
            let index = (id as usize).checked_sub(1)
                .ok_or("el generador devolvio ID cero")?;
            if index >= TYPES {
                return Err(format!("el generador devolvio ID fuera de rango: {id}"));
            }
            let class = tier_index(catalog.outcomes[index].payout_bps);
            let slot = current_index as usize % ring.len();

            if current_index < train_end {
                train_id_counts[index] += 1;
                if train_id_counts[index] > blind_count {
                    blind_count = train_id_counts[index];
                    blind_id = id;
                }
                class_counts[class] += 1;
                if let Some(prev) = previous {
                    trained_successor[prev as usize - 1] = id;
                }
                if let Some(prev) = previous_class {
                    first[prev][class] += 1;
                }
                if let (Some(prev2), Some(prev1)) = (penultimate_class, previous_class) {
                    second[prev2][prev1][class] += 1;
                }
            } else {
                test_id_counts[index] += 1;
                id_blind.check(blind_id, id);
                if let Some(prev) = previous {
                    id_increment.check(prev % TYPES as u32 + 1, id);
                    id_repeat.check(prev, id);
                    let learned = trained_successor[prev as usize - 1];
                    if learned != 0 {
                        id_markov.check(learned, id);
                    }
                }
                if current_index >= 100 {
                    id_lag100.check(ring[(current_index as usize - 100) % ring.len()], id);
                }
                if current_index >= 1000 {
                    id_lag1000.check(ring[(current_index as usize - 1000) % ring.len()], id);
                }

                let training_n = train_end as f64;
                let mut prior = [0.0; CLASS_COUNT];
                for (j, p) in prior.iter_mut().enumerate() {
                    *p = (class_counts[j] as f64 + 1.0)
                        / (training_n + CLASS_COUNT as f64);
                }
                let baseline_pred = max_probability(&prior);
                baseline.observe(class, baseline_pred, prior[class], prior[class]);

                if let Some(prev) = previous_class {
                    let row_sum: u64 = first[prev].iter().sum();
                    let denom = row_sum as f64 + SMOOTHING_1;
                    let mut probability = [0.0; CLASS_COUNT];
                    for j in 0..CLASS_COUNT {
                        probability[j] = (first[prev][j] as f64 + SMOOTHING_1 * prior[j]) / denom;
                    }
                    markov1.observe(
                        class,
                        max_probability(&probability),
                        probability[class],
                        prior[class],
                    );
                }
                if let (Some(prev2), Some(prev1)) = (penultimate_class, previous_class) {
                    let row_sum: u64 = second[prev2][prev1].iter().sum();
                    let denom = row_sum as f64 + SMOOTHING_2;
                    let mut probability = [0.0; CLASS_COUNT];
                    for j in 0..CLASS_COUNT {
                        probability[j] =
                            (second[prev2][prev1][j] as f64 + SMOOTHING_2 * prior[j]) / denom;
                    }
                    markov2.observe(
                        class,
                        max_probability(&probability),
                        probability[class],
                        prior[class],
                    );
                }
                if let Some(prev) = previous_class {
                    test_pairs[prev][class] += 1;
                }

                // Los lags y los limites entre paquetes se evalúan en
                // el test holdout, con independencia del training.
                for (index, &lag) in LAGS.iter().enumerate() {
                    if current_index >= lag as u64 {
                        lag_reports[index].sample(
                            id,
                            ring[(current_index as usize - lag) % ring.len()],
                        );
                    }
                }
                if offset == 0 && current_index > 0 {
                    boundary_count += 1;
                    if previous == Some(id) {
                        boundary_repeats += 1;
                    }
                }
            }
            ring[slot] = id;
            penultimate_class = previous_class;
            previous_class = Some(class);
            previous = Some(id);
            current_index += 1;
        }
    }

    let mut predictor_reports = vec![
        id_blind.report("ID mas frecuente aprendido en training"),
        id_increment.report("Siguiente ID = ID previo + 1"),
        id_repeat.report("Siguiente ID = mismo ID anterior"),
        id_markov.report("Ultimo sucesor observado para ese ID (training congelado)"),
        id_lag100.report("Repetir ID hace 100 giros"),
        id_lag1000.report("Repetir ID hace 1000 giros"),
    ];
    // Bonferroni adjustment is effectively more conservative than 5 sigma
    // across the six simple ID predictors.
    predictor_reports.sort_by(|a, b| a.name.cmp(&b.name));

    let category_reports = vec![
        baseline.report("Distribucion de 8 niveles aprendida en training"),
        markov1.report("Markov de nivel de premio, orden 1"),
        markov2.report("Markov de nivel de premio, orden 2"),
    ];

    let analyzed_lags: Vec<LagReport> = lag_reports
        .into_iter()
        .zip(LAGS)
        .map(|(stats, lag)| stats.report(lag))
        .collect();

    // Chi-square global of equal ID frequencies in holdout: only use
    // large-sample approximation if expected count per ID >= 5.
    let expected = test_len as f64 / TYPES as f64;
    let type_chi_z = if expected >= 5.0 {
        let chi: f64 = test_id_counts.iter().map(|&count| {
            (count as f64 - expected).powi(2) / expected
        }).sum();
        let df = (TYPES - 1) as f64;
        Some((chi - df) / (2.0 * df).sqrt())
    } else {
        None
    };

    let class_prior: [f64; CLASS_COUNT] =
        std::array::from_fn(|j| (class_counts[j] as f64 + 1.0)
            / (train_end as f64 + CLASS_COUNT as f64));
    let pairs_total: u64 = test_pairs.iter().flat_map(|row| row.iter()).sum();
    let chi_transition = if pairs_total > 0 {
        // Compare observed next-tier frequencies with fixed train priors
        // for each previous-tier row. Approx. 8*(8-1)=56 degrees of freedom.
        let chi: f64 = test_pairs.iter().map(|row| {
            let row_n: u64 = row.iter().sum();
            row.iter().enumerate().map(|(j, &count)| {
                let expected = row_n as f64 * class_prior[j];
                if expected >= 5.0 {
                    (count as f64 - expected).powi(2) / expected
                } else {
                    0.0
                }
            }).sum::<f64>()
        }).sum();
        let df = 56.0_f64;
        Some((chi - df) / (2.0 * df).sqrt())
    } else {
        None
    };

    let is_flagged = predictor_reports.iter().any(|r| r.statistically_significant_excess)
        || category_reports.iter().any(|r| r.statistically_significant_improvement)
        || analyzed_lags.iter().any(|r| r.significant_correlation)
        || type_chi_z.is_some_and(|z| z.abs() > THRESHOLD_Z)
        || chi_transition.is_some_and(|z| z > THRESHOLD_Z);
    let mut warnings = vec![
        "Predictores evaluados fuera de muestra (70% entrenamiento, 30% prueba).".to_owned(),
        "Umbral heuristico de 5 sigmas; no garantiza criptoseguridad ni cubre todos los ataques."
            .to_owned(),
        "Pocas coincidencias de ID con 128000 tipos: evitar conclusiones por unos pocos aciertos."
            .to_owned(),
        "Correlaciones y valores chi-cuadrado son aproximados; revisar efectos de pruebas multiples."
            .to_owned(),
    ];
    if is_flagged {
        warnings.push(
            "Al menos una prueba supero el umbral de alerta. Repetir con nuevas semillas, \
             mayores muestras y revision manual antes de atribuir predictibilidad."
                .to_owned(),
        );
    } else {
        warnings.push(
            "Los predictores evaluados no superaron el umbral de alarma. \
             Esto NO demuestra que el CSPRNG sea impredecible para todos los adversarios."
                .to_owned(),
        );
    }

    Ok(LongAttackReport {
        mode,
        observations,
        training_observations: train_end,
        holdout_observations: test_len,
        training_fraction: train_end as f64 / observations as f64,
        elapsed_seconds: start.elapsed().as_secs_f64(),
        id_predictors: predictor_reports,
        payout_class_predictors: category_reports,
        lags: analyzed_lags,
        holdout_type_chi_square_z_approx: type_chi_z,
        holdout_type_chi_square_eligible: type_chi_z.is_some(),
        first_order_payout_chi_square_z_approx: chi_transition,
        observed_package_boundaries: boundary_count,
        boundary_repeat_hits: boundary_repeats,
        expected_boundary_repeat_hits: boundary_count as f64 / TYPES as f64,
        any_flagged_statistical_pattern: is_flagged,
        warnings,
    })
}

fn max_probability(p: &[f64; CLASS_COUNT]) -> usize {
    p.iter().enumerate()
        .max_by(|a, b| a.1.total_cmp(b.1))
        .map(|(index, _)| index)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    #[test]
    fn long_stream_detects_deliberately_weak_counter() {
        let mut rng = rand::rngs::StdRng::seed_from_u64(101);
        let catalog = crate::generate_catalog(&mut rng).unwrap();
        let report = observe_long_sequence(&catalog, Mode::WeakCounter, 30_000, &mut rng).unwrap();
        let predictor = report.id_predictors.iter()
            .find(|p| p.name.contains("ID previo + 1")).unwrap();
        assert!(predictor.accuracy_percent > 98.0);
        assert!(report.any_flagged_statistical_pattern);
    }

    #[test]
    fn long_stream_independent_draws_have_full_catalog_and_valid_holdout() {
        let mut rng = rand::rngs::StdRng::seed_from_u64(202);
        let catalog = crate::generate_catalog(&mut rng).unwrap();
        let report = observe_long_sequence(
            &catalog,
            Mode::WithReplacement,
            30_000,
            &mut rng,
        ).unwrap();
        assert_eq!(report.training_observations, 21_000);
        assert_eq!(report.holdout_observations, 9_000);
        assert_eq!(report.id_predictors.len(), 6);
        assert_eq!(report.payout_class_predictors.len(), 3);
        assert_eq!(report.lags.len(), 6);
        assert_eq!(report.observed_package_boundaries, 90);
        assert!(!report.holdout_type_chi_square_eligible);
        assert!(observe_long_sequence(&catalog, Mode::WithReplacement, 99, &mut rng).is_err());
    }
}
