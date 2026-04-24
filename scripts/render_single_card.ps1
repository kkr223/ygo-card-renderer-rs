param(
    [string]$CardCode = "2511",
    [string]$CardName = "",
    [string]$Label = "single",
    [string]$Language = "sc",
    [string]$ArtImage = "",

    [string]$OutFrameEnabled = "true",
    [string]$OutFrameImage = "D:\workspace\ygo\ygoworkspace\ygo-card-renderer-rs\resources\front.png",
    [int]$OutFrameX = 0,
    [int]$OutFrameY = 0,
    [switch]$DisableOutFrameEffect,
    [ValidateSet("", "eblock-border", "eblock-border-o", "original", "colored")]
    [string]$OutFrameEffectBox = "eblock-border-o",
    [string]$OutFrameEffectBackgroundColor = "#ffffff",
    [string]$OutFrameEffectOpacity = "0.75",
    [switch]$DisableOutFrameNameBlock,

    # Generic foreground layer kept for non-out-frame experiments.
    [string]$ForegroundImage = "",
    [int]$ForegroundX = 0,
    [int]$ForegroundY = 0
)

$ErrorActionPreference = "Stop"

$repoRoot = Split-Path -Parent $PSScriptRoot
$cargoArgs = @("test", "--test", "render", "render_single_card_from_cdb", "--", "--nocapture")

function Set-YgoEnvIfSet {
    param(
        [string]$Name,
        [string]$Value
    )

    if (-not [string]::IsNullOrWhiteSpace($Value)) {
        Set-Item ("Env:" + $Name) $Value
    }
}

function Set-YgoEnvBoolIfSet {
    param(
        [string]$Name,
        [string]$Value
    )

    if ([string]::IsNullOrWhiteSpace($Value)) {
        return
    }

    $normalized = $Value.Trim().ToLowerInvariant()
    if ($normalized -notin @("1", "0", "true", "false", "yes", "no", "on", "off")) {
        throw "$Name must be a boolean-like value, got '$Value'"
    }

    Set-Item ("Env:" + $Name) $Value
}

Write-Host "Running single-card render..." -ForegroundColor Cyan
Write-Host "Repo: $repoRoot"
if (-not [string]::IsNullOrWhiteSpace($CardCode)) {
    Write-Host "Card code: $CardCode"
}
if (-not [string]::IsNullOrWhiteSpace($CardName)) {
    Write-Host "Card name query: $CardName"
}
Write-Host "Label: $Label"
Write-Host ""

Push-Location $repoRoot
$originalYgoEnv = @{}
try {
    foreach ($item in Get-ChildItem Env:YGO_* -ErrorAction SilentlyContinue) {
        $originalYgoEnv[$item.Name] = $item.Value
    }

    foreach ($item in Get-ChildItem Env:YGO_* -ErrorAction SilentlyContinue) {
        Remove-Item ("Env:" + $item.Name) -ErrorAction SilentlyContinue
    }

    Set-YgoEnvIfSet "YGO_RENDER_CARD_CODE" $CardCode
    Set-YgoEnvIfSet "YGO_RENDER_CARD_NAME" $CardName
    Set-YgoEnvIfSet "YGO_RENDER_LABEL" $Label
    Set-YgoEnvIfSet "YGO_LANGUAGE" $Language
    Set-YgoEnvIfSet "YGO_ART_IMAGE" $ArtImage

    Set-YgoEnvBoolIfSet "YGO_OUT_FRAME" $OutFrameEnabled
    Set-YgoEnvIfSet "YGO_OUT_FRAME_IMAGE" $OutFrameImage
    if (-not [string]::IsNullOrWhiteSpace($OutFrameImage)) {
        Set-Item Env:YGO_OUT_FRAME_X ([string]$OutFrameX)
        Set-Item Env:YGO_OUT_FRAME_Y ([string]$OutFrameY)
    }
    if ($DisableOutFrameEffect) {
        Set-Item Env:YGO_OUT_FRAME_EFFECT_ENABLED "false"
    }
    Set-YgoEnvIfSet "YGO_OUT_FRAME_EFFECT_BOX" $OutFrameEffectBox
    Set-YgoEnvIfSet "YGO_OUT_FRAME_EFFECT_BACKGROUND_COLOR" $OutFrameEffectBackgroundColor
    Set-YgoEnvIfSet "YGO_OUT_FRAME_EFFECT_OPACITY" $OutFrameEffectOpacity
    if ($DisableOutFrameNameBlock) {
        Set-Item Env:YGO_OUT_FRAME_NAME_BLOCK_ENABLED "false"
    }

    Set-YgoEnvIfSet "YGO_FOREGROUND_IMAGE" $ForegroundImage
    if (-not [string]::IsNullOrWhiteSpace($ForegroundImage)) {
        Set-Item Env:YGO_FOREGROUND_X ([string]$ForegroundX)
        Set-Item Env:YGO_FOREGROUND_Y ([string]$ForegroundY)
    }

    Write-Host "Effective YGO_* environment:" -ForegroundColor Cyan
    $activeYgoEnv = @(Get-ChildItem Env:YGO_* -ErrorAction SilentlyContinue | Sort-Object Name)
    if ($activeYgoEnv.Count -eq 0) {
        Write-Host "  (none)"
    }
    else {
        foreach ($item in $activeYgoEnv) {
            Write-Host "  $($item.Name)=$($item.Value)"
        }
    }
    Write-Host ""

    & cargo @cargoArgs
    if ($LASTEXITCODE -ne 0) {
        throw "cargo test failed with exit code $LASTEXITCODE"
    }
}
finally {
    foreach ($item in Get-ChildItem Env:YGO_* -ErrorAction SilentlyContinue) {
        Remove-Item ("Env:" + $item.Name) -ErrorAction SilentlyContinue
    }

    foreach ($key in $originalYgoEnv.Keys) {
        Set-Item ("Env:" + $key) ([string]$originalYgoEnv[$key])
    }

    Pop-Location
}
