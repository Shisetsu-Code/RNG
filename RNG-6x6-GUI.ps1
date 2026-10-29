param([switch]$SelfTest, [switch]$TestForm)
# Interfaz Windows: 128 resultados RNG 6x6 y N paquetes de 100.
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
[System.Windows.Forms.Application]::EnableVisualStyles()
if ($SelfTest) {
    Write-Output 'GUI_SELF_TEST_OK'
    exit 0
}
$nl = [Environment]::NewLine
$exe = Join-Path $PSScriptRoot 'RNG-6x6-CLI.exe'
if (-not (Test-Path -LiteralPath $exe -PathType Leaf)) {
    if ($TestForm) { throw 'No existe RNG-6x6-CLI.exe; copia el ejecutable Rust.' }
    [void][System.Windows.Forms.MessageBox]::Show(
        ('No se encuentra RNG-6x6-CLI.exe.' + $nl + 'Descomprime TODOS los archivos del ZIP en la misma carpeta.'),
        'RNG - Falta un archivo',
        [System.Windows.Forms.MessageBoxButtons]::OK,
        [System.Windows.Forms.MessageBoxIcon]::Error
    )
    exit 1
}

$form = New-Object System.Windows.Forms.Form
$form.Text = 'RNG 6x6 - Generador de paquetes de 100 apuestas'
$form.ClientSize = New-Object System.Drawing.Size(780, 685)
$form.StartPosition = 'CenterScreen'
$form.FormBorderStyle = 'FixedSingle'
$form.MaximizeBox = $false
$form.AutoScaleMode = 'Dpi'
$form.Font = New-Object System.Drawing.Font('Segoe UI', 10)
$form.BackColor = [System.Drawing.Color]::FromArgb(248, 250, 252)

function Add-Label {
    param([string]$Text, [int]$X, [int]$Y, [int]$Width = 720, [int]$Height = 24)
    $label = New-Object System.Windows.Forms.Label
    $label.Text = $Text
    $label.Location = New-Object System.Drawing.Point($X, $Y)
    $label.Size = New-Object System.Drawing.Size($Width, $Height)
    $form.Controls.Add($label)
    return $label
}
function Add-TextBox {
    param([string]$Value, [int]$X, [int]$Y, [int]$Width)
    $box = New-Object System.Windows.Forms.TextBox
    $box.Location = New-Object System.Drawing.Point($X, $Y)
    $box.Size = New-Object System.Drawing.Size($Width, 30)
    $box.Text = $Value
    $form.Controls.Add($box)
    return $box
}
function Add-CheckBox {
    param([string]$Text, [int]$X, [int]$Y, [int]$Width = 700, [bool]$Checked = $false)
    $check = New-Object System.Windows.Forms.CheckBox
    $check.Location = New-Object System.Drawing.Point($X, $Y)
    $check.Size = New-Object System.Drawing.Size($Width, 29)
    $check.Text = $Text
    $check.Checked = $Checked
    $form.Controls.Add($check)
    return $check
}
function Get-PositiveInteger {
    param([string]$Value, [string]$Name)
    [long]$n = 0
    if (-not [long]::TryParse($Value.Trim(), [ref]$n) -or $n -lt 1) {
        throw "$Name debe ser un numero entero mayor que cero."
    }
    return $n
}

$title = Add-Label 'GENERADOR RNG 6 x 6' 26 18 710 42
$title.Font = New-Object System.Drawing.Font('Segoe UI', 19, [System.Drawing.FontStyle]::Bold)
$subtitle = Add-Label '128 matrices distintas  |  100 apuestas por paquete  |  RTP teorico 96,5%' 28 66
$subtitle.ForeColor = [System.Drawing.Color]::FromArgb(65, 78, 98)

[void](Add-Label '¿Cuantos paquetes queres generar?' 28 114)
$countBox = Add-TextBox '100' 28 146 300
$countHint = Add-Label '100 paquetes = 10.000 apuestas' 348 149 380
$countHint.ForeColor = [System.Drawing.Color]::FromArgb(65, 78, 98)
$countBox.Add_TextChanged({
    [long]$n = 0
    if ([long]::TryParse($countBox.Text, [ref]$n) -and $n -gt 0) {
        $countHint.Text = ('{0:N0} paquetes = {1:N0} apuestas' -f $n, ($n * 100))
    } else {
        $countHint.Text = 'Ingresa un numero valido.'
    }
})

[void](Add-Label 'Carpeta donde guardar el catalogo y los paquetes' 28 199)
$outputBox = Add-TextBox (Join-Path $PSScriptRoot 'salida_rng') 28 232 630
$chooseFolder = New-Object System.Windows.Forms.Button
$chooseFolder.Text = '...'
$chooseFolder.Location = New-Object System.Drawing.Point(674, 230)
$chooseFolder.Size = New-Object System.Drawing.Size(65, 32)
$form.Controls.Add($chooseFolder)
$chooseFolder.Add_Click({
    $dialog = New-Object System.Windows.Forms.FolderBrowserDialog
    $dialog.Description = 'Selecciona la carpeta para guardar los resultados RNG'
    $dialog.SelectedPath = $PSScriptRoot
    if ($dialog.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) {
        $outputBox.Text = $dialog.SelectedPath
    }
    $dialog.Dispose()
})

$reuseCheck = Add-CheckBox 'Reutilizar los mismos 128 tipos si el catalogo ya existe' 28 286 720 $true
$compactCheck = Add-CheckBox 'Paquetes compactos: guardar solo IDs (matrices en catalogo_128.json)' 28 318 730 $false
$splitCheck = Add-CheckBox 'Ademas, generar un archivo JSON por cada paquete' 28 350 720 $false

$startButton = New-Object System.Windows.Forms.Button
$startButton.Text = 'GENERAR PAQUETES'
$startButton.Font = New-Object System.Drawing.Font('Segoe UI', 11, [System.Drawing.FontStyle]::Bold)
$startButton.Location = New-Object System.Drawing.Point(28, 398)
$startButton.Size = New-Object System.Drawing.Size(340, 49)
$startButton.BackColor = [System.Drawing.Color]::FromArgb(35, 103, 218)
$startButton.ForeColor = [System.Drawing.Color]::White
$startButton.FlatStyle = 'Flat'
$form.Controls.Add($startButton)

$cancelButton = New-Object System.Windows.Forms.Button
$cancelButton.Text = 'CANCELAR'
$cancelButton.Location = New-Object System.Drawing.Point(392, 398)
$cancelButton.Size = New-Object System.Drawing.Size(150, 49)
$cancelButton.Enabled = $false
$form.Controls.Add($cancelButton)

$statusLabel = Add-Label 'Listo para generar los paquetes.' 28 459 720
$statusLabel.ForeColor = [System.Drawing.Color]::FromArgb(55, 65, 81)

$progress = New-Object System.Windows.Forms.ProgressBar
$progress.Location = New-Object System.Drawing.Point(28, 489)
$progress.Size = New-Object System.Drawing.Size(711, 13)
$progress.Style = 'Marquee'
$progress.MarqueeAnimationSpeed = 0
$form.Controls.Add($progress)

$output = New-Object System.Windows.Forms.TextBox
$output.Location = New-Object System.Drawing.Point(28, 518)
$output.Size = New-Object System.Drawing.Size(711, 136)
$output.Multiline = $true
$output.ReadOnly = $true
$output.ScrollBars = 'Vertical'
$output.WordWrap = $false
$output.Font = New-Object System.Drawing.Font('Consolas', 9)
$output.Text = 'Se crea un catalogo de 128 matrices 6x6.' + $nl +
    'Cada paquete contiene 100 tipos diferentes, elegidos aleatoriamente.' + $nl +
    'Los paquetes se guardan en paquetes.jsonl (un JSON por linea).'
$form.Controls.Add($output)

$script:process = $null
$script:startedAt = [datetime]::MinValue
$script:stdoutPath = ''
$script:stderrPath = ''

$timer = New-Object System.Windows.Forms.Timer
$timer.Interval = 500
$timer.Add_Tick({
    if ($null -eq $script:process) { return }
    if (-not $script:process.HasExited) {
        $seconds = ([datetime]::Now - $script:startedAt).TotalSeconds
        $statusLabel.Text = ('Generando... {0:N1} segundos. Se puede cancelar.' -f $seconds)
        return
    }
    $timer.Stop()
    $progress.MarqueeAnimationSpeed = 0
    $startButton.Enabled = $true
    $cancelButton.Enabled = $false
    try {
        $exitCode = $script:process.ExitCode
        $logText = if (Test-Path $script:stdoutPath) { [System.IO.File]::ReadAllText($script:stdoutPath) } else { '' }
        $errorText = if (Test-Path $script:stderrPath) { [System.IO.File]::ReadAllText($script:stderrPath) } else { '' }
        if ($exitCode -eq 0) {
            $statusLabel.Text = 'Paquetes generados correctamente.'
            $output.Text = $logText.Substring([Math]::Max(0, $logText.Length - 7000))
        } else {
            $statusLabel.Text = "Error al generar (codigo $exitCode)."
            $output.Text = $errorText + $nl + $logText
            [void][System.Windows.Forms.MessageBox]::Show(
                ('El programa devolvio un error.' + $nl + $errorText),
                'RNG - Error de simulacion'
            )
        }
    } catch {
        $statusLabel.Text = 'No se pudo leer el reporte.'
        $output.Text = $_.Exception.ToString()
    } finally {
        $script:process.Dispose()
        $script:process = $null
    }
})

$startButton.Add_Click({
    try {
        $count = Get-PositiveInteger $countBox.Text 'Cantidad de paquetes'
        $dir = $outputBox.Text.Trim()
        if ([string]::IsNullOrWhiteSpace($dir)) {
            throw 'Ingresa una carpeta de salida.'
        }
        if ($splitCheck.Checked -and $count -gt 10000) {
            $choice = [System.Windows.Forms.MessageBox]::Show(
                'Vas a crear mas de 10.000 archivos individuales. ¿Continuar?',
                'RNG - Muchos archivos',
                [System.Windows.Forms.MessageBoxButtons]::YesNo,
                [System.Windows.Forms.MessageBoxIcon]::Warning
            )
            if ($choice -ne [System.Windows.Forms.DialogResult]::Yes) { return }
        }
        if (-not (Test-Path -LiteralPath $dir)) {
            [void][System.IO.Directory]::CreateDirectory($dir)
        }
        $arguments = '--packages ' + $count + ' --out "' + $dir + '"'
        if ($compactCheck.Checked) { $arguments += ' --compact' }
        if ($splitCheck.Checked) { $arguments += ' --split-files' }
        $previousCatalog = Join-Path $dir 'catalogo_128.json'
        if ($reuseCheck.Checked -and (Test-Path -LiteralPath $previousCatalog)) {
            $arguments += ' --reuse-catalog "' + $previousCatalog + '"'
        }

        $script:stdoutPath = Join-Path $dir 'ejecucion.log'
        $script:stderrPath = Join-Path $dir 'errores.log'
        $script:process = Start-Process -FilePath $exe -ArgumentList $arguments -WorkingDirectory $PSScriptRoot -WindowStyle Hidden -RedirectStandardOutput $script:stdoutPath -RedirectStandardError $script:stderrPath -PassThru
        if ($null -eq $script:process) {
            throw 'Windows no pudo iniciar RNG-6x6-CLI.exe.'
        }
        $script:startedAt = [datetime]::Now
        $startButton.Enabled = $false
        $cancelButton.Enabled = $true
        $progress.MarqueeAnimationSpeed = 25
        $statusLabel.Text = 'Generacion en ejecucion...'
        $output.Text = "Paquetes solicitados: $count" + $nl +
            "Apuestas solicitadas: $($count * 100)" + $nl +
            "Salida: $dir"
        $timer.Start()
    } catch {
        $statusLabel.Text = 'Error de inicio.'
        $output.Text = $_.Exception.ToString()
        [void][System.Windows.Forms.MessageBox]::Show(
            $_.Exception.Message, 'RNG - Error',
            [System.Windows.Forms.MessageBoxButtons]::OK,
            [System.Windows.Forms.MessageBoxIcon]::Error
        )
    }
})

$cancelButton.Add_Click({
    $timer.Stop()
    if ($null -ne $script:process) {
        try {
            if (-not $script:process.HasExited) {
                Stop-Process -Id $script:process.Id -Force -ErrorAction Stop
            }
        } catch {
            $output.Text = $_.Exception.Message
        } finally {
            $script:process.Dispose()
            $script:process = $null
        }
    }
    $progress.MarqueeAnimationSpeed = 0
    $cancelButton.Enabled = $false
    $startButton.Enabled = $true
    $statusLabel.Text = 'Generacion cancelada.'
    $output.Text = 'Cancelado. Algunos archivos parciales pueden permanecer en la carpeta.'
})

$form.Add_FormClosing({
    $timer.Stop()
    if ($null -ne $script:process) {
        try {
            if (-not $script:process.HasExited) {
                Stop-Process -Id $script:process.Id -Force
            }
        } catch {}
        $script:process.Dispose()
        $script:process = $null
    }
})

if ($TestForm) {
    Write-Output 'GUI_FORM_CREATED_OK'
    $form.Dispose()
    exit 0
}
[void][System.Windows.Forms.Application]::Run($form)
