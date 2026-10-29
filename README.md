# RNG 6×6 — Rust

Simulador de 128 saldos iniciales distintos, con una matriz aleatoria de **6 filas × 6 columnas** por giro y **RTP teórico exacto de 96,5%**.

El código no fuerza el RTP observado ni ajusta los premios según el saldo: el saldo **solo** sirve para debitar apuestas, acreditar premios y detener una cartera que ya no pueda apostar.

## Windows — interfaz grafica y ejecutable listo para usar (sin instalar Rust)

1. Entra a [GitHub Actions](https://github.com/Shisetsu-Code/RNG/actions/workflows/ci.yml).
2. Abre la última ejecución **exitosa** de «Rust RNG CI».
3. Abajo, en **Artifacts**, descarga **RNG-6x6-Windows-GUI-x64** (archivo ZIP).
4. Descomprime el ZIP en una carpeta con permiso de escritura.
5. Haz doble clic en **RNG-6x6.exe** para abrir la ventana con los campos, botones de ejecutar/cancelar y resultados. Como alternativa puedes usar **INICIAR-WINDOWS.cmd**.

El programa pregunta, en este orden:

- Cuántas apuestas ejecutar (por ejemplo, 10.000, 1.000.000 o 10.000.000).
- **1** para que esa sea la cantidad **total repartida entre 128 saldos** o **2** para que sea la cantidad **por cada saldo** (hasta 128 veces más apuestas).
- Importe de cada apuesta (múltiplo de 48 unidades mínimas), saldo inicial y separación entre los 128 saldos.
- Si deseas guardar los resultados automáticamente en **resultados.csv**.

El menú muestra RTP teórico y RTP observado, tiempo de ejecución, apuestas jugadas, premios y un ejemplo de matriz 6×6. Si un saldo se queda sin fondos, el programa puede realizar menos apuestas que las solicitadas.

La interfaz principal es grafica, no necesita abrir manualmente la consola. **RNG-6x6-CLI.exe** es la version de consola para terminales y automatizaciones. Si la aplicacion no abre, ejecuta **DIAGNOSTICO-WINDOWS.cmd**: hace una autoprueba y guarda un reporte en la carpeta temporal de Windows. Windows puede advertir que el archivo no esta firmado digitalmente. Extrae todos los archivos juntos antes de ejecutarlo.

## Ejecución desde código fuente (Windows, Linux o macOS)

Requiere Rust estable (mínimo 1.85) y Cargo. Para abrir el menú interactivo desde la carpeta del repositorio:

```bash
cargo run --release
```

Para ejecutar **1.000.000 de apuestas en total** sin menú:

```bash
cargo run --release -- --total-spins 1000000 --bet 48 --csv resultados.csv
```

Para ejecutar **10.000 apuestas por cada saldo** (hasta 1.280.000):

```bash
cargo run --release -- --spins 10000 --bet 48 --csv resultados.csv
```

También puedes ejecutar el .exe con los mismos parámetros, por ejemplo `RNG-6x6-CLI.exe --bets 1000000 --csv resultados.csv`.

## Ejecución anterior compatible

Requiere Rust estable (mínimo 1.85) y Cargo.

```bash
cargo run --release -- --spins 10000 --bet 48 --base-balance 1000000 --balance-step 10000 --csv resultados.csv
```

Genera 128 saldos iniciales:

- Saldo 1 = `base-balance`.
- Saldo n = `base-balance + (n - 1) * balance-step`, para n = 1..128.
- Apuesta fija por giro = `bet` unidades mínimas.
- Se detienen los giros de un saldo cuando no alcanza para la apuesta.
- Con `--total-spins N` la cuota se distribuye uniformemente; si alguna cartera se queda sin fondos, las apuestas pendientes no se reasignan.
- CSV: 128 filas de saldos, giros, apuestas, premios, RTP observado y saldo final.

El ejemplo usa unidades enteras (pueden ser centavos), sin cálculo monetario en punto flotante. El valor de `--bet` debe ser múltiplo de **48** porque las seis líneas usan apuestas iguales y el multiplicador máximo tiene precisión de 1/8.

Si solo interesan totales y la matriz de ejemplo:

```bash
cargo run --release -- --spins 50000 --summary-only
```

Para ver los parámetros: `cargo run -- --help`.

## Cómo se calcula el RTP

Cada una de las 36 celdas contiene uno de **cinco** símbolos uniformes (`0..4`). Se evalúan las **seis líneas horizontales**, buscando la cantidad de símbolos iguales consecutivos desde la columna izquierda. Cada línea recibe 1/6 de la apuesta del giro.

| Símbolos iniciales iguales (exactos) | Probabilidad por línea | Pago sobre apuesta de la línea |
|---|---:|---:|
| 0 o 1 | 80% | 0× |
| 2 | 4/25 = 16% | 3× |
| 3 | 4/125 = 3,2% | 8× |
| 4 | 4/625 = 0,64% | 20× |
| 5 | 4/3125 = 0,128% | 50× |
| 6 | 1/3125 = 0,032% | 115,625× |

**Comprobación exacta:**

```text
RTP = (4/25 × 3) + (4/125 × 8) + (4/625 × 20)
    + (4/3125 × 50) + (1/3125 × 115,625)
    = 0,480 + 0,256 + 0,128 + 0,064 + 0,037
    = 0,965 = 193/200 = 96,5%.
```

La función `exact_rtp()` obtiene también `24125/25000` mediante aritmética **entera**, sin aproximaciones.

El RTP teórico **no significa** que cada cuenta, cada lote de 100 giros o una simulación finita entregue exactamente 96,5%. La variación aleatoria puede ser significativa.

## Aleatoriedad y balances

El programa usa `rand 0.10.3`, con la opción `unbiased`, y `rand::rng()`, un generador criptográfico inicializado con entropía del sistema operativo y reseeding periódico. Cada símbolo se obtiene mediante muestreo uniforme sin sesgo. Ni el ID, ni el saldo ni la cantidad apostada se usan como semillas o filtros para seleccionar matrices.

Las 128 carteras comparten el mismo proceso de simulación, pero las matrices se generan con nuevas extracciones del CSPRNG. Los resultados de un saldo no modifican la distribución de los demás.

Este repositorio es un **prototipo de simulación**, no un motor de apuestas reales auditado o certificado. Para producción deben añadirse registros verificables de giros, gestión y rotación de semillas, controles de concurrencia, límites operativos y una auditoría independiente del RNG y del juego.

## Pruebas

```bash
cargo test
cargo clippy --all-targets -- -D warnings
```

Las pruebas verifican la fracción exacta del RTP, premios deducidos de la matriz, premios máximos, rechazos de entradas inválidas, símbolos válidos, los 128 saldos distintos y la suspensión de apuestas por insuficiencia de saldo.

La carpeta `.github/workflows` contiene una verificación automática en GitHub Actions.
