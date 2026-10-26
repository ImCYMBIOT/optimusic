<#
  bench.ps1 — measure CPU and memory of a whole app (main process + all child processes).

  Usage (from the folder this file is in):
    powershell -ExecutionPolicy Bypass -File .\bench.ps1 -App tauri    -Phase playing
    powershell -ExecutionPolicy Bypass -File .\bench.ps1 -App electron -Phase idle -Seconds 60

  -App    firefox | electron | tauri   (or any process-name pattern, e.g. "*Music*")
  -Phase  any label: idle, playing, hidden ...
  Results are appended to bench-results.csv and the full table is printed at the end.
#>
param(
    [Parameter(Mandatory = $true)][string]$App,
    [Parameter(Mandatory = $true)][string]$Phase,
    [int]$Seconds = 180,
    [int]$Interval = 2,
    [string]$Out = "bench-results.csv"
)

$patterns = @{ firefox = 'firefox*'; electron = '*youtube*music*'; tauri = 'optimusic*' }
if ($patterns.ContainsKey($App)) { $pattern = $patterns[$App] } else { $pattern = $App }
$cores = [Environment]::ProcessorCount

# All PIDs belonging to the app: processes whose name matches, plus every descendant
# (this is how WebView2's msedgewebview2.exe and Electron/Firefox helpers get counted).
function Get-AppTree {
    $all = Get-CimInstance Win32_Process -Property ProcessId, ParentProcessId, Name
    $children = @{}
    $matching = @{}
    foreach ($p in $all) {
        $parent = [int]$p.ParentProcessId
        if (-not $children.ContainsKey($parent)) { $children[$parent] = New-Object System.Collections.ArrayList }
        [void]$children[$parent].Add([int]$p.ProcessId)
        if ($p.Name -like $pattern) { $matching[[int]$p.ProcessId] = $true }
    }
    $ids = New-Object 'System.Collections.Generic.HashSet[int]'
    $queue = New-Object System.Collections.Queue
    foreach ($p in $all) {
        if ($p.Name -like $pattern -and -not $matching.ContainsKey([int]$p.ParentProcessId)) {
            $queue.Enqueue([int]$p.ProcessId)
        }
    }
    while ($queue.Count -gt 0) {
        $id = $queue.Dequeue()
        if ($id -ne 0 -and $ids.Add($id) -and $children.ContainsKey($id)) {
            foreach ($c in $children[$id]) { $queue.Enqueue($c) }
        }
    }
    return , $ids
}

# Private working set = the "Memory" column in Task Manager.
function Get-PrivateMemMB($ids) {
    $sum = 0
    foreach ($p in (Get-CimInstance Win32_PerfFormattedData_PerfProc_Process -Property IDProcess, WorkingSetPrivate)) {
        if ($ids.Contains([int]$p.IDProcess)) { $sum += [double]$p.WorkingSetPrivate }
    }
    return $sum / 1MB
}

# CPU time from the raw performance counter (units of 100 ns). Unlike Get-Process,
# this works for sandboxed child processes (WebView2/Electron/Firefox renderers and audio).
function Get-CpuSeconds($ids) {
    $map = @{}
    foreach ($p in (Get-CimInstance Win32_PerfRawData_PerfProc_Process -Property IDProcess, PercentProcessorTime)) {
        $id = [int]$p.IDProcess
        if ($ids.Contains($id)) { $map[$id] = [double]$p.PercentProcessorTime / 1e7 }
    }
    return $map
}

$ids = Get-AppTree
if ($ids.Count -eq 0) {
    Write-Host "No running processes match '$pattern'. Is the app open?" -ForegroundColor Red
    Write-Host "Find its process name in Task Manager > Details, then pass it, e.g. -App '*Music*'"
    exit 1
}
$names = $ids | ForEach-Object { (Get-Process -Id $_ -ErrorAction SilentlyContinue).ProcessName } |
    Group-Object | ForEach-Object { "$($_.Name) x$($_.Count)" }
Write-Host "Measuring '$App' / '$Phase' for $Seconds s ($($ids.Count) processes: $($names -join ', '))" -ForegroundColor Cyan
Write-Host "Don't touch the app until it finishes." -ForegroundColor Yellow

$prevCpu = Get-CpuSeconds $ids
$prevTime = Get-Date
$start = $prevTime
$mem = @(); $cpu = @(); $counts = @()

while (((Get-Date) - $start).TotalSeconds -lt $Seconds) {
    Start-Sleep -Seconds $Interval
    $ids = Get-AppTree
    $now = Get-Date
    $curCpu = Get-CpuSeconds $ids
    $delta = 0.0
    foreach ($id in $curCpu.Keys) {
        if ($prevCpu.ContainsKey($id)) { $delta += [math]::Max(0, $curCpu[$id] - $prevCpu[$id]) }
    }
    $wall = ($now - $prevTime).TotalSeconds
    $cpuPct = 100.0 * $delta / ($wall * $cores)
    $memMB = Get-PrivateMemMB $ids
    $mem += $memMB; $cpu += $cpuPct; $counts += $ids.Count
    $elapsed = [int]($now - $start).TotalSeconds
    Write-Host ("  {0,4}s  mem {1,7:N0} MB   cpu {2,5:N2} %   procs {3}" -f $elapsed, $memMB, $cpuPct, $ids.Count)
    $prevCpu = $curCpu; $prevTime = $now
}

$row = [pscustomobject]@{
    Time       = (Get-Date).ToString("yyyy-MM-dd HH:mm")
    App        = $App
    Phase      = $Phase
    Seconds    = $Seconds
    Processes  = ($counts | Measure-Object -Maximum).Maximum
    AvgMemMB   = [math]::Round(($mem | Measure-Object -Average).Average, 0)
    PeakMemMB  = [math]::Round(($mem | Measure-Object -Maximum).Maximum, 0)
    AvgCpuPct  = [math]::Round(($cpu | Measure-Object -Average).Average, 2)
    PeakCpuPct = [math]::Round(($cpu | Measure-Object -Maximum).Maximum, 2)
}
$row | Export-Csv -Path $Out -NoTypeInformation -Append
Write-Host "`nAll results so far ($Out):" -ForegroundColor Green
Import-Csv $Out | Format-Table -AutoSize
