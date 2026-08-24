$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
$iconsDir = Join-Path $projectRoot 'node_modules\lucide-react\dist\esm\icons'
$outFile = Join-Path $projectRoot 'src\main\app-gpui\src\ui\icons_data.rs'

# Component names used across src/renderer (Icon suffix stripped).
$components = @'
Accessibility
AlertCircle
Aperture
AppWindow
ArrowUp
ArrowUpDown
ArrowUpRight
Ban
Box
Camera
Check
CheckCircle
ChevronDown
ChevronLeft
ChevronRight
Circle
Clock
Cloud
CloudUpload
Code2
Copy
Crop
Download
Droplets
Edit3
Eraser
ExternalLink
EyeOff
FileUp
Film
FolderOpen
Frame
Gauge
Globe
Grid3X3
HardDrive
Hash
Heart
HelpCircle
Highlighter
History
Image
ImageOff
Info
Keyboard
LayoutGrid
LayoutList
ListOrdered
Loader2
Maximize2
Mic
MicOff
Minus
Monitor
MonitorDot
MousePointer2
MousePointerClick
Music
Palette
PanelRightClose
PanelRightOpen
Pause
PenLine
Pencil
Pin
Pipette
Play
Plus
Power
QrCode
RefreshCcw
RefreshCw
RotateCcw
RotateCw
Save
Scale
Scan
ScanText
Scissors
Scroll
Search
Settings
Shield
Shuffle
Smartphone
Square
SquareDashed
Star
Subtitles
TextCursor
TimerReset
Trash2
TriangleAlert
Type
Upload
Video
VideoOff
Volume2
VolumeX
Wallpaper
Webcam
X
XCircle
ZoomIn
'@ -split "`n" | ForEach-Object { $_.Trim() } | Where-Object { $_ }

function ToKebab([string]$name) {
    # Handle runs of capitals first: XCircle -> X-Circle, Grid3X3 -> Grid3-X3.
    $name = $name -creplace '([A-Z])([A-Z][a-z])', '$1-$2'
    $name = $name -creplace '([a-z0-9])([A-Z])', '$1-$2'
    return $name.ToLower()
}

# Explicit file-name aliases for renamed lucide icons.
$aliases = @{
    'alert-circle'      = @('circle-alert', 'alert-circle')
    'x-circle'          = @('circle-x', 'x-circle')
    'check-circle'      = @('circle-check', 'check-circle')
    'help-circle'       = @('circle-help', 'help-circle', 'circle-question-mark')
    'pen-line'          = @('pen-line')
    'grid3-x3'          = @('grid-3x3', 'grid-3-x-3')
    'code2'             = @('code-2', 'code-xml')
    'edit3'             = @('pen-line', 'edit-3')
    'loader2'           = @('loader-2', 'loader-circle')
    'maximize2'         = @('maximize-2')
    'mouse-pointer2'    = @('mouse-pointer-2')
    'trash2'            = @('trash-2')
    'volume2'           = @('volume-2')
    'subtitles'         = @('captions', 'subtitles')
    'triangle-alert'    = @('triangle-alert', 'alert-triangle')
    'square-dashed'     = @('square-dashed', 'box-select')
    'pin'               = @('pin')
    'scan-text'         = @('scan-text')
    'cloud-upload'      = @('upload-cloud', 'cloud-upload')
    'refresh-ccw'       = @('refresh-ccw')
    'image-off'         = @('image-off')
    'layout-grid'       = @('layout-grid')
    'layout-list'       = @('layout-list')
    'list-ordered'      = @('list-ordered')
    'panel-right-close' = @('panel-right-close')
    'panel-right-open'  = @('panel-right-open')
    'zoom-in'           = @('zoom-in')
    'rotate-ccw'        = @('rotate-ccw')
    'rotate-cw'         = @('rotate-cw')
    'file-up'           = @('file-up')
    'folder-open'       = @('folder-open')
    'history'           = @('history', 'rotate-ccw-clock')
}

function GetAttr($attrs, [string]$k) {
    if ($attrs.ContainsKey($k)) { return $attrs[$k] }
    return $null
}

# SVG treats the first moveto of a path as absolute whatever its case, but the
# coordinate pairs that follow a lowercase `m` are relative linetos. These
# sub-paths get concatenated into one `d`, where a leading `m` would resolve
# against the end of the previous sub-path instead of the origin — so promote
# the moveto to `M` and re-attach the rest as an explicit relative `l`.
function NormalizeLeadingMoveTo([string]$d) {
    $d = $d.Trim()
    if (-not $d.StartsWith('m')) { return $d }
    $match = [regex]::Match($d, '^m[\s,]*(-?[\d.]+)[\s,]+(-?[\d.]+)(.*)$')
    if (-not $match.Success) { return 'M' + $d.Substring(1) }
    $x = $match.Groups[1].Value
    $y = $match.Groups[2].Value
    $rest = $match.Groups[3].Value
    if ($rest -match '^[\s,]*-?[\d.]') { $rest = ' l' + $rest }
    return "M$x $y$rest"
}

function ConvertNodeToPath($tag, $attrs) {
    $list = New-Object System.Collections.Generic.List[string]
    switch ($tag) {
        'path' {
            $d = GetAttr $attrs 'd'
            if ($d) {
                $list.Add((NormalizeLeadingMoveTo $d))
            }
        }
        'line' {
            $x1 = [double](GetAttr $attrs 'x1'); $y1 = [double](GetAttr $attrs 'y1')
            $x2 = [double](GetAttr $attrs 'x2'); $y2 = [double](GetAttr $attrs 'y2')
            $list.Add("M $x1 $y1 L $x2 $y2")
        }
        'polyline' {
            $pts = (GetAttr $attrs 'points') -split '\s+' | Where-Object { $_ }
            $list.Add('M ' + ($pts -join ' L '))
        }
        'polygon' {
            $pts = (GetAttr $attrs 'points') -split '\s+' | Where-Object { $_ }
            $list.Add('M ' + ($pts -join ' L ') + ' Z')
        }
        'circle' {
            $cx = [double](GetAttr $attrs 'cx'); $cy = [double](GetAttr $attrs 'cy'); $r = [double](GetAttr $attrs 'r')
            $list.Add("M $($cx - $r) $cy A $r $r 0 1 1 $($cx + $r) $cy A $r $r 0 1 1 $($cx - $r) $cy")
        }
        'ellipse' {
            $cx = [double](GetAttr $attrs 'cx'); $cy = [double](GetAttr $attrs 'cy')
            $rx = [double](GetAttr $attrs 'rx'); $ry = [double](GetAttr $attrs 'ry')
            $list.Add("M $($cx - $rx) $cy A $rx $ry 0 1 1 $($cx + $rx) $cy A $rx $ry 0 1 1 $($cx - $rx) $cy")
        }
        'rect' {
            $x = [double](GetAttr $attrs 'x'); $y = [double](GetAttr $attrs 'y')
            $w = [double](GetAttr $attrs 'width'); $h = [double](GetAttr $attrs 'height')
            $rxs = GetAttr $attrs 'rx'; $rys = GetAttr $attrs 'ry'
            if (-not $rxs -and -not $rys) {
                $list.Add("M $x $y H $($x + $w) V $($y + $h) H $x Z")
            } else {
                if (-not $rxs) { $rxs = $rys }
                if (-not $rys) { $rys = $rxs }
                $rx = [double]$rxs; $ry = [double]$rys
                $list.Add("M $($x + $rx) $y H $($x + $w - $rx) A $rx $ry 0 0 1 $($x + $w) $($y + $ry) V $($y + $h - $ry) A $rx $ry 0 0 1 $($x + $w - $rx) $($y + $h) H $($x + $rx) A $rx $ry 0 0 1 $x $($y + $h - $ry) V $($y + $ry) A $rx $ry 0 0 1 $($x + $rx) $y Z")
            }
        }
        default {
            Write-Host "  !! unhandled tag: $tag"
        }
    }
    return ,$list
}
function ParseAttrs([string]$body) {
    $attrs = @{}
    # Matches key: "value" or key: value (number)
    $rx = [regex]'([a-zA-Z][a-zA-Z0-9-]*)\s*:\s*(?:"([^"]*)"|([-0-9.eE]+))'
    foreach ($m in $rx.Matches($body)) {
        $value = if ($m.Groups[2].Success) { $m.Groups[2].Value } else { $m.Groups[3].Value }
        $attrs[$m.Groups[1].Value] = $value
    }
    return $attrs
}

$results = [ordered]@{}
$missing = @()

foreach ($component in $components) {
    $kebab = ToKebab $component
    $candidates = @($kebab)
    if ($aliases.ContainsKey($kebab)) { $candidates = $aliases[$kebab] }

    $file = $null
    foreach ($candidate in $candidates) {
        $path = Join-Path $iconsDir "$candidate.mjs"
        if (Test-Path $path) {
            # Re-export stubs (export { default } from ...) have no icon data;
            # keep looking at the next candidate.
            if ((Get-Content $path -Raw) -match '__iconNode') { $file = $path; break }
        }
    }

    if (-not $file) {
        $missing += "$component ($kebab)"
        continue
    }

    $content = Get-Content $file -Raw
    # Extract the __iconNode array body.
    $start = $content.IndexOf('const __iconNode =')
    if ($start -lt 0) {
        # Some files declare it differently; fall back to the first array literal
        # assigned to __iconNode anywhere in the file.
        $start = $content.IndexOf('__iconNode')
    }
    if ($start -lt 0) {
        $missing += "$component (no __iconNode)"
        continue
    }
    $arrayStart = $content.IndexOf('[', $start)
    # Find matching closing bracket by counting.
    $depth = 0; $end = -1
    for ($i = $arrayStart; $i -lt $content.Length; $i++) {
        $c = $content[$i]
        if ($c -eq '[') { $depth++ }
        elseif ($c -eq ']') { $depth--; if ($depth -eq 0) { $end = $i; break } }
    }
    $nodeBody = $content.Substring($arrayStart, $end - $arrayStart + 1)

    # Split top-level element tuples.
    $elements = @()
    $d = 0; $tupleStart = -1
    for ($i = 0; $i -lt $nodeBody.Length; $i++) {
        $c = $nodeBody[$i]
        if ($c -eq '[') { $d++; if ($d -eq 2) { $tupleStart = $i + 1 } }
        elseif ($c -eq ']') { if ($d -eq 2) { $elements += $nodeBody.Substring($tupleStart, $i - $tupleStart) }; $d-- }
    }

    $paths = New-Object System.Collections.Generic.List[string]
    foreach ($element in $elements) {
        $tagMatch = [regex]::Match($element, '^\s*"([a-z]+)"')
        if (-not $tagMatch.Success) { continue }
        $tag = $tagMatch.Groups[1].Value
        $attrBody = $element.Substring($tagMatch.Index + $tagMatch.Length)
        $attrs = ParseAttrs $attrBody
        $converted = ConvertNodeToPath $tag $attrs
        foreach ($entry in $converted) {
            if ($entry) { $paths.Add($entry) }
        }
    }

    if ($paths.Count -eq 0) {
        $missing += "$component (no paths)"
        continue
    }

    $results[$component] = $paths -join ' '
}

# Emit Rust.
$sb = New-Object System.Text.StringBuilder
[void]$sb.AppendLine('// Generated from lucide-react v1.32.0 icon path data (ISC license).')
[void]$sb.AppendLine('// Each entry is the concatenated `d` attribute of every sub-shape, in a')
[void]$sb.AppendLine('// 24x24 viewBox designed for a round stroke of width 2.')
[void]$sb.AppendLine('')
$table = New-Object System.Collections.Generic.List[string]
foreach ($name in $results.Keys) {
    $rustName = (ToKebab $name) -replace '-', '_'
    $upper = $rustName.ToUpper()
    $escaped = $results[$name] -replace '\\', '\\\\' -replace '"', '\"'
    [void]$sb.AppendLine("pub const ${upper}: &str = `"$escaped`";")
    $table.Add("    (`"$rustName`", ${upper}),")
}
[void]$sb.AppendLine('')
[void]$sb.AppendLine('pub static ICONS: &[(&str, &str)] = &[')
foreach ($entry in $table) { [void]$sb.AppendLine($entry) }
[void]$sb.AppendLine('];')
Set-Content -Path $outFile -Value ($sb.ToString() -join "`n") -Encoding UTF8

Write-Host ""
Write-Host "Generated $($results.Count) icons -> $outFile"
if ($missing.Count -gt 0) {
    Write-Host "MISSING:" -ForegroundColor Red
    $missing | ForEach-Object { Write-Host "  $_" -ForegroundColor Red }
}
