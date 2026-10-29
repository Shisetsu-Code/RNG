param([switch]$SelfTest)
# Native Windows Forms interface for the Rust RNG simulator.
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
    [void][System.Windows.Forms.MessageBox]::Show(
        ("No se encuentra RNG-6x6-CLI.exe." + $nl + $nl + "Extrae TODOS los archivos del ZIP en la misma carpeta."),
        'RNG 6x6 - Archivo faltante',
        [System.Windows.Forms.MessageBoxButtons]::OK,
        [System.Windows.Forms.MessageBoxIcon]::Error
    )
    exit 1
}

$form = New-Object System.Windows.Forms.Form
$form.Text = 'RNG 6x6 | Simulador local de 128 saldos'
$form.ClientSize = New-Object System.Drawing.Size(740, 670)
$form.StartPosition = [System.Windows.Forms.FormStartPosition]::CenterScreen
$form.FormBorderStyle = [System.Windows.Forms.FormBorderStyle]::FixedSingle
$form.MaximizeBox = $false
$form.AutoScaleMode = [System.Windows.Forms.AutoScaleMode]::Dpi
$form.Font = New-Object System.Drawing.Font('Segoe UI', 10)
$form.BackColor = [System.Drawing.Color]::FromArgb(248, 250, 252)

function AddLabel {
    param([string]$Text, [int]$Top, [int]$Left = 25, [int]$Width = 680)
    $label = New-Object System.Windows.Forms.Label
    $label.Text = $Text
    $label.Location = New-Object System.Drawing.Point($Left, $Top)
    $label.Size = New-Object System.Drawing.Size($Width, 26)
    $form.Controls.Add($label)
    return $label
}

function AddTextBox {
    param([string]$Value, [int]$Top, [int]$Left = 25, [int]$Width = 315)
    $box = New-Object System.Windows.Forms.TextBox
    $box.Text = $Value
    $box.Location = New-Object System.Drawing.Point($Left, $Top)
    $box.Size = New-Object System.Drawing.Size($Width, 28)
    $form.Controls.Add($box)
    return $box
}

$title = AddLabel 'SIMULADOR RNG 6 x 6' 18
$title.Font = New-Object System.Drawing.Font('Segoe UI', 17, [System.Drawing.FontStyle]::Bold)
$title.Height = 42
$subtitle = AddLabel 'RTP teorico: 96,5%   |   128 saldos   |   Ejecucion local sin internet' 63
$subtitle.ForeColor = [System.Drawing.Color]::FromArgb(75, 85, 99)

[void](AddLabel 'Cantidad de apuestas' 107)
$countBox = AddTextBox '10000' 137 25 330
$totalRadio = New-Object System.Windows.Forms.RadioButton
$totalRadio.Text = 'Total entre los 128 saldos'
$totalRadio.Location = New-Object System.Drawing.Point(375, 139)
$totalRadio.Size = New-Object System.Drawing.Size(320, 27)
$totalRadio.Checked = $true
$form.Controls.Add($totalRadio)
$perWalletRadio = New-Object System.Windows.Forms.RadioButton
$perWalletRadio.Text = 'Esa cantidad para CADA saldo'
$perWalletRadio.Location = New-Object System.Drawing.Point(375, 170)
$perWalletRadio.Size = New-Object System.Drawing.Size(320, 27)
$form.Controls.Add($perWalletRadio)

[void](AddLabel 'Importe por apuesta (multiplo de 48)' 206 25 320)
$betBox = AddTextBox '48' 236 25 315
[void](AddLabel 'Saldo inicial del primer monedero' 206 375 315)
$balanceBox = AddTextBox '1000000' 236 375 315
[void](AddLabel 'Incremento entre saldos iniciales' 283 25 350)
$stepBox = AddTextBox '10000' 314 25 315

$csvCheck = New-Object System.Windows.Forms.CheckBox
$csvCheck.Text = 'Guardar resultados.csv junto al programa'
$csvCheck.Location = New-Object System.Drawing.Point(375, 313)
$csvCheck.Size = New-Object System.Drawing.Size(340, 29)
$csvCheck.Checked = $true
$form.Controls.Add($csvCheck)

$startButton = New-Object System.Windows.Forms.Button
$startButton.Text = 'EJECUTAR SIMULACION'
$startButton.Font = New-Object System.Drawing.Font('Segoe UI', 10, [System.Drawing.FontStyle]::Bold)
$startButton.Location = New-Object System.Drawing.Point(25, 367)
$startButton.Size = New-Object System.Drawing.Size(330, 44)
$startButton.BackColor = [System.Drawing.Color]::FromArgb(36, 99, 235)
$startButton.ForeColor = [System.Drawing.Color]::White
$startButton.FlatStyle = [System.Windows.Forms.FlatStyle]::Flat
$form.Controls.Add($startButton)

$cancelButton = New-Object System.Windows.Forms.Button
$cancelButton.Text = 'CANCELAR'
$cancelButton.Location = New-Object System.Drawing.Point(375, 367)
$cancelButton.Size = New-Object System.Drawing.Size(150, 44)
$cancelButton.Enabled = $false
$form.Controls.Add($cancelButton)

$statusLabel = AddLabel 'Preparado para ejecutar.' 426
$statusLabel.ForeColor = [System.Drawing.Color]::FromArgb(55, 65, 81)

$progress = New-Object System.Windows.Forms.ProgressBar
$progress.Location = New-Object System.Drawing.Point(25, 454)
$progress.Size = New-Object System.Drawing.Size(680, 12)
$progress.Style = [System.Windows.Forms.ProgressBarStyle]::Marquee
$progress.MarqueeAnimationSpeed = 0
$form.Controls.Add($progress)

$output = New-Object System.Windows.Forms.TextBox
$output.Multiline = $true
$output.ReadOnly = $true
$output.ScrollBars = [System.Windows.Forms.ScrollBars]::Vertical
$output.WordWrap = $false
$output.Font = New-Object System.Drawing.Font('Consolas', 9)
$output.Location = New-Object System.Drawing.Point(25, 482)
$output.Size = New-Object System.Drawing.Size(680, 157)
$output.Text = "Elige una cantidad y haz clic en EJECUTAR SIMULACION." + $nl + "Puedes cancelar una ejecucion desde esta ventana." + $nl + "Un total puede ser menor al solicitado si no alcanza el saldo."
$form.Controls.Add($output)

$script:process = $null
$script:startedAt = [datetime]::MinValue

function ReadPositiveUlong {
    param([string]$InputText, [string]$Description, [uint64]$Minimum)
    [uint64]$value = 0
    if (-not [uint64]::TryParse($InputText.Trim(), [ref]$value) -or $value -lt $Minimum) {
        throw "$Description debe ser un numero entero mayor o igual a $Minimum."
    }
    return $value
}

$timer = New-Object System.Windows.Forms.Timer
$timer.Interval = 250
$timer.Add_Tick({
    if ($null -eq $script:process) { return }
    if (-not $script:process.HasExited) {
        $elapsed = ([datetime]::Now - $script:startedAt).TotalSeconds
        $statusLabel.Text = ('Simulando... {0:N1} segundos. Puedes cancelar.' -f $elapsed)
        return
    }
    $timer.Stop()
    $progress.MarqueeAnimationSpeed = 0
    $startButton.Enabled = $true
    $cancelButton.Enabled = $false
    try {
        $stdout = $script:process.StandardOutput.ReadToEnd()
        $stderr = $script:process.StandardError.ReadToEnd()
        $exitCode = $script:process.ExitCode
        if ($exitCode -eq 0) {
            $statusLabel.Text = 'Completado correctamente.'
            $output.Text = $stdout
        } else {
            $statusLabel.Text = "Error de ejecucion (codigo $exitCode)."
            $output.Text = ($stderr + $nl + $stdout)
        }
    } catch {
        $statusLabel.Text = 'No fue posible leer la salida.'
        $output.Text = $_.Exception.Message
    } finally {
        $script:process.Dispose()
        $script:process = $null
    }
})

$startButton.Add_Click({
    try {
        [uint64]$count = ReadPositiveUlong $countBox.Text 'Cantidad de apuestas' 1
        [uint64]$bet = ReadPositiveUlong $betBox.Text 'Importe por apuesta' 48
        [uint64]$balance = ReadPositiveUlong $balanceBox.Text 'Saldo inicial' 0
        [uint64]$step = ReadPositiveUlong $stepBox.Text 'Incremento entre saldos' 1
        if ($bet % 48 -ne 0) {
            throw 'El importe por apuesta debe ser multiplo de 48.'
        }
        if ($perWalletRadio.Checked -and $count -gt ([uint64]::MaxValue / 128)) {
            throw 'La cantidad de apuestas por saldo es demasiado grande.'
        }

        $mode = if ($totalRadio.Checked) { '--total-spins' } else { '--spins' }
        $parameters = "$mode $count --bet $bet --base-balance $balance --balance-step $step --summary-only"

        if ($csvCheck.Checked) {
            $csvFile = Join-Path $PSScriptRoot 'resultados.csv'
            $parameters += ' --csv "' + $csvFile + '"'
        }
        $psi = New-Object System.Diagnostics.ProcessStartInfo
        $psi.FileName = $exe
        $psi.Arguments = $parameters
        $psi.WorkingDirectory = $PSScriptRoot
        $psi.UseShellExecute = $false
        $psi.CreateNoWindow = $true
        $psi.RedirectStandardOutput = $true
        $psi.RedirectStandardError = $true
        $psi.StandardOutputEncoding = [System.Text.Encoding]::UTF8
        $psi.StandardErrorEncoding = [System.Text.Encoding]::UTF8

        $script:process = New-Object System.Diagnostics.Process
        $script:process.StartInfo = $psi
        if (-not $script:process.Start()) {
            throw 'Windows no pudo iniciar el simulador Rust.'
        }
        $script:startedAt = [datetime]::Now
        $startButton.Enabled = $false
        $cancelButton.Enabled = $true
        $progress.MarqueeAnimationSpeed = 25
        $output.Text = "Calculando..." + $nl + "Cantidad solicitada: $count" + $nl + "Modo: $mode" + $nl + "El RNG no cambia sus probabilidades por saldo."
        $statusLabel.Text = 'Simulacion en ejecucion...'
        $timer.Start()
    } catch {
        $statusLabel.Text = 'Error al iniciar la simulacion.'
        $output.Text = $_.Exception.ToString()
        [void][System.Windows.Forms.MessageBox]::Show(
            $_.Exception.Message, 'RNG 6x6 - Error',
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
                $script:process.Kill()
                $script:process.WaitForExit(2000) | Out-Null
            }
        } catch {
            $output.Text = $_.Exception.Message
        } finally {
            $script:process.Dispose()
            $script:process = $null
        }
    }
    $progress.MarqueeAnimationSpeed = 0
    $startButton.Enabled = $true
    $cancelButton.Enabled = $false
    $statusLabel.Text = 'Simulacion cancelada.'
    $output.Text = 'Cancelada. Puedes iniciar otra simulacion.'
})

$form.Add_FormClosing({
    $timer.Stop()
    if ($null -ne $script:process) {
        try {
            if (-not $script:process.HasExited) { $script:process.Kill() }
        } catch {}
        $script:process.Dispose()
        $script:process = $null
    }
})
[void][System.Windows.Forms.Application]::Run($form)
