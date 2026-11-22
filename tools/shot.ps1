param(
    [Parameter(Mandatory)][string]$Out,     # PNG path; the floating widget goes to <Out>-float.png
    [string]$Page = "timer",                # timer | tasks | subjects | history | stats | settings
    [string]$DataDir = "",                  # scratch data folder (STUDYTRACKER_DATA_DIR); never the real one
    [int]$RunSecs = -1,                     # >= 0: start with a running focus block this far in
    [string]$Cat = "",                     # hold Meng in one pose: walk | sleep | groom | stretch | play
    [switch]$Float,                         # capture the floating widget instead of the main window
    [string]$Exe = ""
)
# Renders StudyTracker on one page to a PNG from the app's own frames (other windows on top don't matter),
# then the app quits by itself. Screenshot runs never write data.
if (-not $Exe) { $Exe = (Resolve-Path "$PSScriptRoot\..\target\release\studytracker.exe").Path }
$env:STUDYTRACKER_PAGE = $Page
$env:STUDYTRACKER_SHOT = [System.IO.Path]::GetFullPath($Out)
if ($DataDir) { $env:STUDYTRACKER_DATA_DIR = $DataDir }
if ($RunSecs -ge 0) { $env:STUDYTRACKER_DEMO_RUN = "$RunSecs" }
if ($Float) { $env:STUDYTRACKER_SHOT_FLOAT = "1" }
if ($Cat) { $env:STUDYTRACKER_DEMO_CAT = $Cat }
try {
    $p = Start-Process $Exe -PassThru
    if (-not $p.WaitForExit(25000)) { Stop-Process -Id $p.Id -Force; Write-Error "timed out" }
} finally {
    Remove-Item Env:STUDYTRACKER_PAGE, Env:STUDYTRACKER_SHOT, Env:STUDYTRACKER_DEMO_RUN, Env:STUDYTRACKER_SHOT_FLOAT, Env:STUDYTRACKER_DEMO_CAT -ErrorAction SilentlyContinue
    if ($DataDir) { Remove-Item Env:STUDYTRACKER_DATA_DIR -ErrorAction SilentlyContinue }
}
if (Test-Path $Out) { "saved $Out" } else { Write-Error "no image written" }
