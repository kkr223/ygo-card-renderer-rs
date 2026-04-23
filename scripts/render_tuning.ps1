# 用法：
# 1. 直接编辑下面这些变量
# 2. 在仓库根目录执行（必须用 pwsh，不能用 powershell 5.x）：
#    pwsh -ExecutionPolicy Bypass -File .\scripts\render_tuning.ps1

$ErrorActionPreference = "Stop"

# 二选一：优先用 code，其次 name 模糊匹配
$YGO_RENDER_CARD_CODE = "483"                      # 示例卡号
$YGO_RENDER_CARD_NAME = ""                         # 留空表示不用名称模糊匹配

# 输出文件后缀，会生成到 export/<code>-<name>-<label>.png
$YGO_RENDER_LABEL = "tuning"                       # 示例标签

# 其他基础选项
$YGO_LANGUAGE = "sc"                               # 原值: sc
$YGO_SCALE = "1.0"                                 # 原值: 1.0
$YGO_ART_IMAGE = ""                                # 原值: 无自定义图片
$YGO_TITLE_WIDTH_COMPRESS = "true"                     # 原值: false，可填 true/false
$YGO_DESCRIPTION_FIRST_LINE_COMPRESS = ""          # 原值: false，可填 true/false

# 标题区
$YGO_NAME_TOP = "107"                              # 原值: 97
$YGO_NAME_SIZE = ""                                # 原值: 108
$YGO_NAME_X = "103"                                # 原值: 116
$YGO_TITLE_MAX_WIDTH_WITH_ATTRIBUTE = ""           # 原值: 1033
$YGO_TITLE_MAX_WIDTH_WITHOUT_ATTRIBUTE = ""        # 原值: 1161
$YGO_TITLE_LETTER_SPACING = ""                     # 原值: 0.0

# 类型 / 效果行
$YGO_TYPE_TOP = ""                                 # 原值: 254
$YGO_TYPE_SIZE = ""                                # 原值: 76
$YGO_TYPE_LETTER_SPACING = ""                      # 原值: 2.0
$YGO_EFFECT_TOP = ""                               # 原值: 1528
$YGO_EFFECT_SIZE = ""                              # 原值: 44
$YGO_EFFECT_LINE_HEIGHT = ""                       # 原值: 1.2
$YGO_EFFECT_X = ""                                 # 原值: 109
$YGO_EFFECT_LETTER_SPACING = ""                    # 原值: 2.0
$YGO_EFFECT_TEXT_INDENT = ""                       # 原值: 0

# 正文区
$YGO_DESCRIPTION_SIZE = ""                         # 原值: 36
$YGO_DESCRIPTION_LINE_HEIGHT = ""                  # 原值: 1.2
$YGO_DESCRIPTION_X = ""                            # 原值: 109
$YGO_DESCRIPTION_LETTER_SPACING = ""               # 原值: 2.0
$YGO_BODY_MAX_WIDTH = ""                       # 原值: 1175

# 灵摆
$YGO_PENDULUM_DESCRIPTION_TOP = ""                 # 原值: 1282
$YGO_PENDULUM_DESCRIPTION_SIZE = ""                # 原值: 36

# 数值区
$YGO_STAT_ATK_X = ""                               # 原值: 999
$YGO_STAT_DEF_X = ""                               # 原值: 1282
$YGO_STAT_LINK_X = "1274"                              # 原值: 1280
$YGO_STAT_TOP = "1846"                                 # 原值: 1839
$YGO_STAT_SIZE = ""                                # 原值: 62
$YGO_STAT_LETTER_SPACING = ""                      # 原值: 2.0
$YGO_LINK_TOP = "1860"                                 # 原值: 1855
$YGO_LINK_SIZE = ""                                # 原值: 44

# 版权行位置（right = 距卡片右边缘距离，y = 顶部偏移）
$YGO_COPYRIGHT_RIGHT = ""                          # 原值: 141
$YGO_COPYRIGHT_Y = "1939"                              # 原值: 1936
# 版权行文本内容（留空则不显示）
$YGO_COPYRIGHT_TEXT = "© 1996 KAZUKI "                           # 示例: "© 1996 KAZUKI TAKAHASHI"

# 卡包编码文本 y 坐标（普通/灵摆/link 三种变体）
$YGO_PACKAGE_Y = "1458"                                # 原值: 1455
$YGO_PACKAGE_Y_PENDULUM = ""                       # 原值: 1859
$YGO_PACKAGE_Y_LINK = ""                           # 原值: 1455
# 卡包编码文本内容（留空则不显示）
$YGO_PACKAGE_TEXT = "RC04-JP000"                             # 示例: "RC04-JP000"

# 左下角 ID（密码）文本位置
$YGO_PASSWORD_X = ""                               # 原值: 66
$YGO_PASSWORD_Y = "1937"                               # 原值: 1932

$repoRoot = Split-Path -Parent $PSScriptRoot
$cargoArgs = @("test", "render_single_card_for_tuning", "--", "--ignored", "--nocapture")

function Set-YgoEnvIfSet {
    param(
        [string]$Name,
        [string]$Value
    )

    if (-not [string]::IsNullOrWhiteSpace($Value)) {
        Set-Item ("Env:" + $Name) $Value
    }
}

Write-Host "Running tuning render test..." -ForegroundColor Cyan
Write-Host "Repo: $repoRoot"
Write-Host "Card code: $YGO_RENDER_CARD_CODE"
Write-Host "Card name query: $YGO_RENDER_CARD_NAME"
Write-Host "Label: $YGO_RENDER_LABEL"
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

    Set-YgoEnvIfSet "YGO_RENDER_CARD_CODE" $YGO_RENDER_CARD_CODE
    Set-YgoEnvIfSet "YGO_RENDER_CARD_NAME" $YGO_RENDER_CARD_NAME
    Set-YgoEnvIfSet "YGO_RENDER_LABEL" $YGO_RENDER_LABEL
    Set-YgoEnvIfSet "YGO_LANGUAGE" $YGO_LANGUAGE
    Set-YgoEnvIfSet "YGO_SCALE" $YGO_SCALE
    Set-YgoEnvIfSet "YGO_ART_IMAGE" $YGO_ART_IMAGE
    Set-YgoEnvIfSet "YGO_TITLE_WIDTH_COMPRESS" $YGO_TITLE_WIDTH_COMPRESS
    Set-YgoEnvIfSet "YGO_DESCRIPTION_FIRST_LINE_COMPRESS" $YGO_DESCRIPTION_FIRST_LINE_COMPRESS
    Set-YgoEnvIfSet "YGO_NAME_TOP" $YGO_NAME_TOP
    Set-YgoEnvIfSet "YGO_NAME_SIZE" $YGO_NAME_SIZE
    Set-YgoEnvIfSet "YGO_NAME_X" $YGO_NAME_X
    Set-YgoEnvIfSet "YGO_TITLE_MAX_WIDTH_WITH_ATTRIBUTE" $YGO_TITLE_MAX_WIDTH_WITH_ATTRIBUTE
    Set-YgoEnvIfSet "YGO_TITLE_MAX_WIDTH_WITHOUT_ATTRIBUTE" $YGO_TITLE_MAX_WIDTH_WITHOUT_ATTRIBUTE
    Set-YgoEnvIfSet "YGO_TITLE_LETTER_SPACING" $YGO_TITLE_LETTER_SPACING
    Set-YgoEnvIfSet "YGO_TYPE_TOP" $YGO_TYPE_TOP
    Set-YgoEnvIfSet "YGO_TYPE_SIZE" $YGO_TYPE_SIZE
    Set-YgoEnvIfSet "YGO_TYPE_LETTER_SPACING" $YGO_TYPE_LETTER_SPACING
    Set-YgoEnvIfSet "YGO_EFFECT_TOP" $YGO_EFFECT_TOP
    Set-YgoEnvIfSet "YGO_EFFECT_SIZE" $YGO_EFFECT_SIZE
    Set-YgoEnvIfSet "YGO_EFFECT_LINE_HEIGHT" $YGO_EFFECT_LINE_HEIGHT
    Set-YgoEnvIfSet "YGO_EFFECT_X" $YGO_EFFECT_X
    Set-YgoEnvIfSet "YGO_EFFECT_LETTER_SPACING" $YGO_EFFECT_LETTER_SPACING
    Set-YgoEnvIfSet "YGO_EFFECT_TEXT_INDENT" $YGO_EFFECT_TEXT_INDENT
    Set-YgoEnvIfSet "YGO_DESCRIPTION_SIZE" $YGO_DESCRIPTION_SIZE
    Set-YgoEnvIfSet "YGO_DESCRIPTION_LINE_HEIGHT" $YGO_DESCRIPTION_LINE_HEIGHT
    Set-YgoEnvIfSet "YGO_DESCRIPTION_X" $YGO_DESCRIPTION_X
    Set-YgoEnvIfSet "YGO_DESCRIPTION_LETTER_SPACING" $YGO_DESCRIPTION_LETTER_SPACING
    Set-YgoEnvIfSet "YGO_BODY_MAX_WIDTH" $YGO_BODY_MAX_WIDTH
    Set-YgoEnvIfSet "YGO_PENDULUM_DESCRIPTION_TOP" $YGO_PENDULUM_DESCRIPTION_TOP
    Set-YgoEnvIfSet "YGO_PENDULUM_DESCRIPTION_SIZE" $YGO_PENDULUM_DESCRIPTION_SIZE
    Set-YgoEnvIfSet "YGO_STAT_ATK_X" $YGO_STAT_ATK_X
    Set-YgoEnvIfSet "YGO_STAT_DEF_X" $YGO_STAT_DEF_X
    Set-YgoEnvIfSet "YGO_STAT_LINK_X" $YGO_STAT_LINK_X
    Set-YgoEnvIfSet "YGO_STAT_TOP" $YGO_STAT_TOP
    Set-YgoEnvIfSet "YGO_STAT_SIZE" $YGO_STAT_SIZE
    Set-YgoEnvIfSet "YGO_STAT_LETTER_SPACING" $YGO_STAT_LETTER_SPACING
    Set-YgoEnvIfSet "YGO_LINK_TOP" $YGO_LINK_TOP
    Set-YgoEnvIfSet "YGO_LINK_SIZE" $YGO_LINK_SIZE
    Set-YgoEnvIfSet "YGO_COPYRIGHT_RIGHT" $YGO_COPYRIGHT_RIGHT
    Set-YgoEnvIfSet "YGO_COPYRIGHT_Y" $YGO_COPYRIGHT_Y
    Set-YgoEnvIfSet "YGO_COPYRIGHT_TEXT" $YGO_COPYRIGHT_TEXT
    Set-YgoEnvIfSet "YGO_PACKAGE_Y" $YGO_PACKAGE_Y
    Set-YgoEnvIfSet "YGO_PACKAGE_Y_PENDULUM" $YGO_PACKAGE_Y_PENDULUM
    Set-YgoEnvIfSet "YGO_PACKAGE_Y_LINK" $YGO_PACKAGE_Y_LINK
    Set-YgoEnvIfSet "YGO_PACKAGE_TEXT" $YGO_PACKAGE_TEXT
    Set-YgoEnvIfSet "YGO_PASSWORD_X" $YGO_PASSWORD_X
    Set-YgoEnvIfSet "YGO_PASSWORD_Y" $YGO_PASSWORD_Y

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
