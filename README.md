# RNG 6×6 — 128.000 tipos y atacante de secuencias largas

Proyecto en Rust para generar **128.000 matrices únicas de 6×6**, cada una con un premio propio, y producir paquetes de **100 apuestas** con tipos repetidos permitidos.

**RTP teórico exacto: 96,5%.** El resultado de cada apuesta se selecciona de manera independiente y uniforme entre los 128.000 tipos. No se compensan pérdidas ni se fuerzan frecuencias iguales; cada tipo vuelve a estar disponible en la próxima apuesta. Los identificadores son enteros de **32 bits**.

## Windows 11: ejecución

1. Abre <https://github.com/Shisetsu-Code/RNG/actions/workflows/ci.yml> y selecciona la ejecución reciente que haya terminado con éxito.
2. En **Artifacts**, descarga **RNG-6x6-128000-LONG-Windows-x64**.
3. Extrae el ZIP completo.
4. Abre **RNG-6x6.exe** para generar paquetes o **AUDITAR-RNG.cmd** para lanzar el atacante y configurar la longitud de observación.

No se requiere instalar Rust. La GUI es nativa de Windows y el motor está programado en Rust. Los resultados del generador se guardan en `salida_rng`.

## Generador

Para 10.000 paquetes, que contienen un millón de apuestas:

```powershell
.\RNG-6x6-CLI.exe --packages 10000 --out salida_rng
```

El formato compacto es predeterminado: cada apuesta guarda el ID y el catálogo `catalogo_128000.json` contiene todas las matrices 6×6. Con `--full` se incluye la matriz en cada apuesta; con `--split-files` también se guarda un JSON individual por paquete.

Reutilizar exactamente el mismo catálogo:

```powershell
.\RNG-6x6-CLI.exe --packages 10000 --reuse-catalog salida_rng\catalogo_128000.json --out salida_nueva
```

Archivos: `catalogo_128000.json`, `catalogo_128000.csv`, `paquetes.jsonl`, `resumen_paquetes.csv`, `frecuencias_tipos.csv`, `reporte.json`.

La versión del catálogo ahora es **3**. Los catálogos de 8.192 y 128 tipos anteriores no se pueden reutilizar directamente.

### Tabla de premios

Se cuentan parejas de símbolos iguales horizontalmente adyacentes en una matriz 6×6. Las cuotas son:

| Parejas iguales | Tipos | Pago |
|---|---:|---:|
| 0–3 | 64.000 | 0× |
| 4 | 32.000 | 1× |
| 5 | 16.000 | 1,5× |
| 6 | 8.000 | 2× |
| 7 | 1.000 | 3,52× |
| 8 | 4.000 | 4× |
| 9 | 2.000 | 8× |
| 10+ | 1.000 | 16× |
| **Total** | **128.000** | **96,5% RTP** |

El retorno agregado de los 128.000 tipos es **1.235.200.000 puntos básicos**:

```text
RTP = 1.235.200.000 / (128.000 × 10.000) = 0,965
```

Cada apuesta selecciona uno de estos tipos con probabilidad `1/128000`, independientemente de las apuestas previas. En un millón de apuestas se esperan **7,8125 apariciones por tipo**. La distribución de una muestra finita varía aleatoriamente y no se ajusta según el pasado.

## Laboratorio: atacante de secuencias largas

El atacante observa **resultados consecutivos, incluso al atravesar miles de paquetes**. Entrena con el primer **70%** de la secuencia, congela el modelo y lo evalúa con el **30% restante**, sin mirar resultados futuros. No tiene acceso a semillas ni al estado interno del CSPRNG.

Los tres modelos disponibles son `independent` (actual), `without` (histórico sin reposición) y `weak` (contador deliberadamente predecible, solo para pruebas).

Para atacar **un millón de resultados por modelo**:

```powershell
.\RNG-6x6-AUDITOR.exe --mode all --packages 1000 --long-bets 1000000 --out auditoria_rng
```

Para analizar **10 millones de resultados consecutivos del generador actual** sin pruebas cortas:

```powershell
.\RNG-6x6-AUDITOR.exe --mode independent --long-only --long-bets 10000000 --out auditoria_10m
```

Para que el atacante lea **el mismo archivo JSONL generado previamente**, en lugar de generar otra secuencia:

```powershell
.\RNG-6x6-AUDITOR.exe --mode independent --long-only --long-bets 1000000 --catalog salida_rng\catalogo_128000.json --observe-file salida_rng\paquetes.jsonl --out auditoria_real
```

El archivo debe contener al menos tantos resultados como se indiquen. Se aceptan paquetes compactos (IDs) y completos (matrices con ID). El lector recorre el archivo línea por línea sin retener millones de giros en RAM.

**Qué intenta descubrir el atacante:**

- ID más frecuente, repetición, incremento `+1`, último sucesor aprendido, y predicción por retardos de 100 o 1.000 giros.
- Patrones del premio, correlaciones y modelos Markov de orden 1 y 2 sobre los ocho niveles de pago.
- Autocorrelaciones en retardos de 1, 2, 8, 32, 100 y 1.000, coincidencias y límites entre paquetes.
- Desviaciones de frecuencia y pruebas condicionales contra los valores esperados del modelo uniforme.

Las pruebas usan evaluación fuera de muestra. Los aciertos extremadamente raros se contrastan mediante una cota binomial conservadora; un pequeño número de coincidencias no implica predictibilidad. Pasar los controles no certifica el RNG.

**Informes generados:** `auditoria.json`, `secuencias_largas.json`, `secuencias_largas_predictores.csv`, `secuencias_largas_correlaciones.csv` y `predicciones.csv`.

El menú `AUDITAR-RNG.cmd` permite escoger la cantidad de paquetes y la cantidad de resultados largos. Si existen `salida_rng/catalogo_128000.json` y `salida_rng/paquetes.jsonl`, también permite analizar la secuencia real guardada.

## GPU

El módulo opcional de `wgpu` usa DirectX 12/Vulkan para analizar retornos condicionales del **modelo histórico sin reposición**. El atacante de secuencias largas funciona en **CPU en streaming**. El RNG, la generación de resultados y las semillas criptográficas no se trasladan a la GPU.

```powershell
.\RNG-6x6-AUDITOR.exe --mode without --packages 100000 --long-bets 0 --gpu --out auditoria_gpu
```

Se valida el shader WGSL con Naga durante CI. El rendimiento con una GPU física debe comprobarse por separado.

## Verificaciones

```bash
cargo test --all-targets
cargo clippy --all-targets -- -D warnings
cargo test --features gpu --bin rng_redteam
```

El CI comprueba el catálogo de 128.000 matrices, el RTP exacto, IDs de 32 bits, los 100 resultados por paquete, repeticiones permitidas, el ataque fuera de muestra y la creación de la GUI Windows.

**Alcance:** simulador de seguridad; no equivale a una certificación criptográfica o de juegos de azar. Para operar con apuestas reales se requieren controles y auditoría independiente del servidor, semillas, transacciones, RNG y su integración.
