# RNG 6×6 — 128 tipos de resultado, paquetes de 100

Este programa implementa un catalogo de **128 tipos distintos de apuesta/resultado**, no 128 cuentas o saldos. Cada tipo contiene una matriz de 6 filas x 6 columnas y un multiplicador de pago calculado a partir de esa matriz.

El programa mezcla los 128 tipos de forma aleatoria, selecciona **100 sin repeticion dentro del paquete**, vuelve a mezclar para el siguiente paquete y repite tantas veces como indiques.

El **RTP teorico es exactamente 96,5%**: no se fuerza que cada paquete, ni que un conjunto finito de paquetes, retorne exactamente 96,5%.

## Windows 11: descargar y usar

1. Entra en [GitHub Actions](https://github.com/Shisetsu-Code/RNG/actions/workflows/ci.yml).
2. Abre la ultima ejecucion que tenga el indicador de exito.
3. Abajo, en **Artifacts**, descarga **RNG-6x6-Paquetes-Windows-x64**.
4. **Descomprime el ZIP completo** a una carpeta.
5. Ejecuta con doble clic **RNG-6x6.exe**.

La ventana nativa de Windows permite especificar la **cantidad de paquetes**, seleccionar la ruta del directorio de salida, reutilizar el catalogo generado en una ejecucion anterior, elegir paquete completo o compacto y crear opcionalmente un JSON separado por paquete. Incluye botones para **generar** y **cancelar**, y muestra los resultados o los errores dentro de la ventana.

**Ejemplo:** 500 paquetes equivalen a 50.000 apuestas/selecciones de la lista de 128 tipos.

No hace falta instalar Rust para ejecutar el ZIP de Windows. **El motor y la interfaz grafica estan programados en Rust nativo para Windows (API Win32). No se necesita PowerShell, Python ni instalar Rust.** El programa funciona localmente y no requiere internet.

Si no se inicia, usa **INICIAR-WINDOWS.cmd** y despues **DIAGNOSTICO-WINDOWS.cmd**. El diagnostico prueba la apertura de controles de la GUI y la generacion de tres paquetes a traves del motor Rust, y crea un archivo de texto en la carpeta temporal. Si sigue fallando, comparte el mensaje exacto y ese archivo.

## Terminal

Desde el codigo fuente, con Rust/Cargo instalados:

```powershell
cargo run --release -- --packages 500 --out salida_rng
```

Para usar el ejecutable de Windows (desde su carpeta):

```powershell
.\RNG-6x6-CLI.exe --packages 500 --out salida_rng
```

Opciones:

| Parametro | Funcion |
|---|---|
| `--packages 500` | Genera 500 paquetes de 100 resultados (50.000 apuestas) |
| `--out ruta` | Directorio de salida (predeterminado: `salida_rng`) |
| `--compact` | Cada paquete contiene solo los 100 IDs; busca las matrices en el catalogo |
| `--split-files` | Ademas de `paquetes.jsonl`, crea un archivo JSON por paquete |
| `--reuse-catalog ruta\catalogo_128.json` | Reutiliza exactamente los mismos 128 tipos sin reconstruir el catalogo |

Sin argumentos, la version CLI pregunta la cantidad de paquetes. La version GUI siempre muestra el campo correspondiente.

## Generacion del catalogo

Cada tipo contiene:

- Un identificador de 1 a 128.
- Una matriz de **6×6** con cinco simbolos posibles (0, 1, 2, 3 y 4).
- Una puntuacion obtenida contando las parejas de simbolos horizontalmente adyacentes e iguales, en todas las filas.
- Un multiplicador derivado de esa puntuacion.

La distribucion fija de los 128 tipos es:

| Parejas iguales | Cantidad de tipos | Pago |
|---|---:|---:|
| 0 a 3 | 64 | 0x |
| 4 | 32 | 1x |
| 5 | 16 | 1,5x |
| 6 | 8 | 2x |
| 7 | 1 | 3,52x |
| 8 | 4 | 4x |
| 9 | 2 | 8x |
| 10 o mas | 1 | 16x |
| **TOTAL** | **128** | **RTP 96,5%** |

Calculo exacto mediante puntos basicos (10.000 = 1x):

```text
RTP = (64*0 + 32*10000 + 16*15000 + 8*20000
      + 1*35200 + 4*40000 + 2*80000 + 1*160000)
      / (128*10000)
    = 1235200 / 1280000
    = 0,965 = 96,5%
```

El conjunto se construye con matrices aleatorias y verificadas: no se repiten matrices y el premio guardado debe concordar con su contenido. Luego se mezclan los tipos. Los 128 tipos son **equiprobables** en cada seleccion.

Este es un ejemplo de tabla de premios para el prototipo. Se puede cambiar, pero deben conservarse la comprobacion del RTP, la unicidad de matrices y la correspondencia entre matriz y premio.

## Empaquetado RNG

Para **cada paquete de 100 apuestas**, el motor:

1. Toma los IDs de los 128 tipos.
2. Aplica una mezcla aleatoria mediante el generador criptografico de `rand` inicializado desde el sistema operativo.
3. Selecciona los primeros 100 IDs (sin reemplazo dentro del paquete).
4. Acumula los pagos y calcula el RTP observado de ese paquete.
5. Escribe el paquete a disco y repite el proceso.

**Importante:** un mismo tipo puede aparecer en paquetes diferentes. Cada tipo tiene probabilidad 100/128 de entrar en cada paquete, y su posicion en el paquete es aleatoria. No se modifica la seleccion para ajustar un RTP historico o un saldo.

## Archivos generados

Todos dentro de la carpeta de salida:

| Archivo | Contenido |
|---|---|
| `catalogo_128.json` | Los 128 tipos, sus matrices 6x6, sus premios y metadatos del RTP |
| `catalogo_128.csv` | Version tabular con las seis filas de cada matriz |
| `paquetes.jsonl` | **Un paquete de 100 apuestas por linea**, en formato JSON |
| `resumen_paquetes.csv` | Cantidad de apuestas, retorno total y RTP observado por paquete |
| `frecuencias_tipos.csv` | Veces que aparece cada uno de los 128 tipos |
| `reporte.json` | Cantidad de paquetes, total de apuestas, RTP global y tiempo |
| `paquetes/paquete_000001.json` | Solo cuando se activa `--split-files`: archivo por paquete |

Por defecto, cada linea de `paquetes.jsonl` trae los **100 registros completos**, incluidas sus matrices 6x6 y pagos. El modo compacto reduce significativamente el tamaño: usa 100 IDs que referencian el catalogo separado.

Los archivos se escriben progresivamente, sin almacenar todos los paquetes en la RAM. Una ejecucion nueva sobre el mismo directorio reemplaza los archivos principales; para conservar ejecuciones anteriores elige otra carpeta.

## Pruebas

```bash
cargo test --all-targets
cargo clippy --all-targets -- -D warnings
```

GitHub Actions verifica las 128 matrices distintas, los conteos de cada nivel de premios, el RTP teorico exacto, 100 IDs sin repeticion por paquete, los JSON y CSV, la reutilizacion del catalogo y el ejecutable Windows.

**Alcance:** simulador matematico y generador local de resultados. No es un RNG certificado para apuestas reales; para operar con dinero real hace falta auditoria y certificacion adecuadas.
