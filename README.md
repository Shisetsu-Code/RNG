# RNG 6×6 — catálogo de 8.192 tipos, paquetes de 100

Proyecto Rust para Windows y terminal. Genera **8.192 matrices únicas de 6×6**, cada una con un premio calculado a partir de sus 36 símbolos, con **RTP teórico exacto de 96,5%**. Después produce tantos paquetes de **100 apuestas** como se solicite.

## Cambios en versión 0.3

- **8.192 tipos** (IDs 1..8192); antes había 128.
- **Muestreo independiente con reposición**: cada una de las 100 apuestas selecciona un ID con probabilidad **1/8192**. Se permiten repeticiones dentro del mismo paquete y entre paquetes.
- El RNG no impone cuotas, ciclos ni compensaciones de ganancias. Ni saldos anteriores ni paquetes consumidos cambian la distribución de la próxima apuesta.
- **Reparto uniforme a largo plazo**: cada ID aparece, en promedio, `total_apuestas / 8192` veces. A corto plazo hay variación aleatoria, que el programa mide y exporta.
- Los 128 tipos antiguos no se reutilizan automáticamente. El archivo compatible es ahora `catalogo_8192.json` y la versión del catálogo es 2.
- El formato compacto (100 IDs por paquete, con sus 8.192 matrices en el catálogo) es ahora el predeterminado, para no duplicar grandes bloques de JSON.

**Ejemplo:** 100.000 paquetes generan 10.000.000 apuestas. La frecuencia esperada por tipo es 10.000.000 / 8192 ≈ **1.220,70** apariciones, pero los conteos individuales no serán exactamente iguales.

### Seguridad matemática

Cada apuesta sigue teniendo los 8.192 tipos disponibles, incluso después de observar 99 resultados del mismo paquete. Esto evita la fuga por descarte de tipos de la versión antigua sin reposición.

Incluso en el modelo histórico sin reposición, con 8192 tipos y solo 100 extracciones, el RTP condicional máximo es 97,6805%, por debajo del 100%; el dato descartado sigue filtrando ligeramente información sobre IDs, pero no ofrece por sí solo una apuesta de valor esperado positivo. 

No garantizamos 96,5% de retorno en cada paquete ni que todos los IDs aparezcan el mismo número de veces: hacerlo mediante cuotas adaptativas introduciría dependencias aprovechables. El RTP es **teórico**, no un objetivo que se fuerce con resultados futuros.

## Descargar para Windows

1. Abre [GitHub Actions](https://github.com/Shisetsu-Code/RNG/actions/workflows/ci.yml).
2. Selecciona la última ejecución **exitosa** del flujo.
3. En la sección **Artifacts**, descarga `RNG-6x6-Paquetes-Windows-x64`.
4. Extrae el ZIP completo en una carpeta de Windows.
5. Ejecuta `RNG-6x6.exe` para abrir la interfaz nativa Rust, elegir el número de paquetes y generar los resultados.

También incluye `INICIAR-WINDOWS.cmd`, `DIAGNOSTICO-WINDOWS.cmd` y el laboratorio de ataque `AUDITAR-RNG.cmd`. No requiere instalar Rust ni PowerShell.

## Uso directo en terminal

```powershell
.\RNG-6x6-CLI.exe --packages 100000 --out salida_rng --compact
```

Para incluir las 36 celdas de cada apuesta en cada paquete, en lugar de solo el ID:

```powershell
.\RNG-6x6-CLI.exe --packages 100 --out salida_completa --full
```

Para reutilizar exactamente el mismo catálogo de tipos en otra generación:

```powershell
.\RNG-6x6-CLI.exe --packages 10000 --reuse-catalog salida_rng\catalogo_8192.json --out otra_ejecucion
```

Opciones:

| Opción | Función |
|---|---|
| `--packages N` | Genera N paquetes de 100 apuestas |
| `--out DIRECTORIO` | Carpeta de salida |
| `--compact` | Guarda 100 IDs por paquete (predeterminado) |
| `--full` | Incluye las matrices y premios completos en cada paquete |
| `--split-files` | Además crea `paquetes/paquete_000001.json`, etc. |
| `--reuse-catalog RUTA` | Reutiliza los 8.192 tipos de una ejecución previa |

También se puede compilar desde el código fuente con `cargo run --release -- --packages 100000`.

## Distribución de premios y RTP

Cada matriz tiene seis filas y seis columnas. Cada celda es uno de cinco símbolos (0..4). Se cuentan las parejas horizontales adyacentes de símbolos iguales y se asigna el multiplicador correspondiente.

| Parejas iguales | Tipos | Multiplicador |
|---|---:|---:|
| 0–3 | 4.096 | 0× |
| 4 | 2.048 | 1× |
| 5 | 1.024 | 1,5× |
| 6 | 512 | 2× |
| 7 | 64 | 3,52× |
| 8 | 256 | 4× |
| 9 | 128 | 8× |
| 10 o más | 64 | 16× |
| **Total** | **8.192** | **RTP 96,5%** |

```text
Retorno esperado =
(4096*0 + 2048*1 + 1024*1,5 + 512*2
 + 64*3,52 + 256*4 + 128*8 + 64*16) / 8192
= 7905,28 / 8192
= 0,965 = 96,5%
```

Los pagos se representan en puntos básicos de enteros (`10000 = 1x`), sin error de precisión en la suma. La suma exacta de premios del catálogo es **79.052.800 puntos básicos**, equivalente a 8.192 × 9.650.

El generador valida que las 8.192 matrices sean únicas, que cada premio corresponda a la matriz y que los recuentos por multiplicador cumplan las cuotas definidas. Después, **cada ID participa con probabilidad idéntica e independiente**; no se seleccionan más resultados favorables o desfavorables a raíz de un saldo o de resultados anteriores.

## Archivos de salida

| Archivo | Contenido |
|---|---|
| `catalogo_8192.json` | Los 8.192 tipos, sus matrices 6×6 y pagos |
| `catalogo_8192.csv` | El catálogo tabular, incluidas las seis filas de cada matriz |
| `paquetes.jsonl` | Una línea JSON por paquete, con 100 IDs (repetidos permitidos) |
| `resumen_paquetes.csv` | Los 100 giros, tipos únicos, tipos repetidos, RTP de cada paquete |
| `frecuencias_tipos.csv` | Cuántas veces se utilizó cada ID |
| `reporte.json` | RTP observado total y métricas de distribución |
| `paquetes/paquete_000001.json` | Opcional con `--split-files` |

`reporte.json` incorpora `expected_occurrences_per_type`, `types_observed`, `minimum_occurrences_per_type`, `maximum_occurrences_per_type`, `maximum_frequency_deviation_z` y `duplicate_draws_within_packages`.

La desviación típica de cada frecuencia bajo el modelo uniforme es:

```text
N = total de apuestas
p = 1/8192
media = N*p
desviacion_estandar = sqrt(N*p*(1-p))
```

Para muestras pequeñas, que haya muchos tipos sin aparecer o frecuencias desiguales es normal.

## Auditor de seguridad (generador contra atacante)

Ejecuta `AUDITAR-RNG.cmd`, o desde terminal:

```powershell
.\RNG-6x6-AUDITOR.exe --packages 10000 --mode all --catalog salida_rng\catalogo_8192.json --out auditoria_rng
```

El auditor compara **tres modelos**:

1. `independent`: **el generador actual** con reposición; mantiene 8.192 resultados posibles y RTP condicional de 96,5%.
2. `without`: el **modelo anterior** sin reposición, como contrafactual para demostrar la fuga por descarte de tipos.
3. `weak`: un contador deliberadamente inseguro, para comprobar si el atacante detecta predicción trivial.

Se registran aciertos de predictores, anomalías de frecuencia, RTP observado y oportunidades de ventaja matemática.

**GPU opcional**: utiliza `wgpu` (DirectX 12/Vulkan) para calcular el RTP condicional del modelo histórico sin reposición en lotes. El CSPRNG y los sorteos del generador **siguen en CPU**, por seguridad. Se compara la GPU con el cálculo CPU y se rechazan discrepancias.

```powershell
.\RNG-6x6-AUDITOR.exe --packages 100000 --mode without --gpu --out auditoria_gpu
```

La compilación y validación del shader no equivalen a probarlo en una tarjeta física; para medir rendimiento en una AMD RX 6800 XT hay que ejecutarlo localmente. Puede que para paquetes de 100 la CPU sea más rápida debido a la transferencia de datos.

Los resultados del atacante se guardan en `auditoria.json` y `predicciones.csv`.

## Pruebas

```bash
cargo test --all-targets
cargo clippy --all-targets -- -D warnings
cargo test --features gpu --bin rng_redteam
```

GitHub Actions comprueba los 8.192 tipos únicos, el RTP, el muestreo con reposición, los contadores de distribución, los CSV/JSON, la interfaz Windows y la validación del shader WGSL.

**Limitación:** esto es un simulador y banco de pruebas, no un RNG certificado para apuestas reales. Las pruebas estadísticas no prueban seguridad criptográfica; faltan auditoría independiente, seguridad del servidor, gestión de claves/semillas y certificaciones pertinentes.
