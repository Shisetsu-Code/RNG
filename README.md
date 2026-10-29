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


## Auditoria adversarial: generador vs atacante (Rust)

El segundo ejecutable es **RNG-6x6-AUDITOR.exe**, incluido en el ZIP de Windows.
Para usarlo con una interfaz simple de consola, abre **AUDITAR-RNG.cmd**.
Permite indicar cuántos paquetes de 100 giros analizar y seleccionar
los modelos que se van a comparar.

Ejemplo Windows, desde una terminal:

```powershell
.\RNG-6x6-AUDITOR.exe --packages 100000 --mode all --out auditoria_rng
```

Analisis acelerado en GPU (experimental, con DX12 o Vulkan):

```powershell
.\RNG-6x6-AUDITOR.exe --packages 100000 --mode without --gpu --gpu-batch 50000 --out auditoria_gpu
```

Con el codigo fuente:

```bash
cargo run --release --bin rng_redteam -- --packages 10000 --mode all
cargo run --release --features gpu --bin rng_redteam -- --packages 100000 --mode without --gpu
```

La auditoria prueba **tres modelos distintos**:

1. **sin reposicion:** el modelo actual, barajar 128 IDs y entregar 100 sin repeticion;
2. **independiente con reposicion:** cada giro extrae uniformemente uno de los 128 tipos, sin descartar anteriores;
3. **contador debil:** control negativo, expresamente inseguro y solo de laboratorio, que permite predecir el siguiente tipo mediante +1.

Los ataques usan solo el catalogo y las apuestas YA OBSERVADAS.
Se evalua una prediccion ciega, otra que excluye IDs ya observados y
otra de incremento secuencial; el generador debil tiene que ser detectado.
Las probabilidades se contrastan contra la linea base matematica.

**Vulnerabilidad estructural conocida:** con 99 resultados de un paquete
sin reposicion observados, quedan exactamente 29 IDs posibles para el
resultado numero 100. La probabilidad de una prediccion correcta basada
en exclusion es entonces 1/29, frente a 1/128 antes de empezar.
Ademas, el RTP condicional para un proximo giro puede calcularse como:

```text
RTP_condicional = (suma_premios_catalogo - suma_premios_observados)
                   / (128 - numero_observado) / apuesta_unidad
```

El atacante puede identificar posiciones en las que el proximo giro
tiene valor esperado positivo, si conoce el catalogo completo y el historial,
y si la plataforma le permite decidir si, cuanto o cuando apostar.
La auditoria registra la cantidad de esas oportunidades y el resultado
teorico y realizado cuando se seleccionan.

Esto es un problema del **muestreo sin reposicion**, NO una demostracion de
que el generador criptografico de `rand`/ChaCha haya sido comprometido.
Si el requisito principal es evitar apuestas selectivas basadas en informacion
anterior, se recomienda usar extracciones independientes **con reposicion**:
se mantienen los 128 tipos posibles y el 96,5% de RTP teorico en cada
apuesta, aunque pueden repetirse IDs dentro de un paquete.
No cambia el mecanismo productivo por defecto; el auditor muestra ambas
opciones para tomar una decision informada.

El auditor genera:

- `auditoria.json`: resultados por modelo, riesgos, RTP y medidas estadisticas;
- `predicciones.csv`: prefijos 0/25/50/75/90/99, acierto de predictores,
  tasas esperadas y oportunidades de apuestas con RTP condicional >100%.

### GPU: alcance y limitaciones

`wgpu 30.0.1` usa shaders WGSL y backends nativos DirectX 12/Vulkan.
El analizador GPU calcula en paralelo el retorno condicional por prefijo
para lotes de paquetes **generados por CPU con un CSPRNG**.
Los resultados se comparan contra el calculo CPU (hasta 1024 trazas por
ejecucion). Si hay diferencias, la auditoria falla.

Las tareas que se benefician mas de GPU son las mediciones masivas y
paralelas. La mezcla de 128 IDs, el RNG criptografico y la escritura
de archivos siguen en CPU, y la aceleracion neta **no esta garantizada**:
mide tiempos y transferencias para determinar si compensa. La GPU
no debe generar semillas ni resultados monetarios de produccion.

El backend GPU es opcional. Si no se detecta una GPU compatible o falla
la inicializacion, ejecuta el mismo estudio sin `--gpu`. En GitHub Actions
se compila el backend GPU, pero solo puede verificarse su rendimiento en
un equipo con tarjeta grafica disponible.

### Validez de la auditoria

Pasar pruebas estadisticas no certifica un CSPRNG. Esta herramienta detecta
algunos fallos de frecuencia, prediccion trivial, sesgos y oportunidades
de seleccion, pero no sustituye pruebas estandarizadas, revision del
protocolo y auditoria criptografica independiente. Nunca deben
entregarse lotes futuros, semillas o el estado interno del generador a clientes.

