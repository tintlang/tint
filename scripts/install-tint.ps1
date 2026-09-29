$ErrorActionPreference = "Stop"

$repo = if ($env:TINT_REPO) { $env:TINT_REPO } else { "tintlang/tint" }
$installDir = if ($env:TINT_INSTALL_DIR) { $env:TINT_INSTALL_DIR } else { Join-Path $env:USERPROFILE "bin" }

if ($env:PROCESSOR_ARCHITECTURE -ne "AMD64") {
    throw "Only Windows x86_64 is currently supported. Download another asset from https://github.com/$repo/releases"
}

$release = Invoke-RestMethod "https://api.github.com/repos/$repo/releases/latest"
$tag = $release.tag_name
$archive = "tint-$tag-x86_64-pc-windows-msvc.zip"
$baseUrl = "https://github.com/$repo/releases/download/$tag"
$tempDir = Join-Path ([System.IO.Path]::GetTempPath()) ("tint-" + [guid]::NewGuid())
$zipPath = Join-Path $tempDir $archive

New-Item -ItemType Directory -Force $tempDir | Out-Null
try {
    Invoke-WebRequest "$baseUrl/$archive" -OutFile $zipPath
    $checksumPath = "$zipPath.sha256"
    Invoke-WebRequest "$baseUrl/$archive.sha256" -OutFile $checksumPath

    $expected = (Get-Content $checksumPath -Raw).Split([char]::WhiteSpace)[0]
    $actual = (Get-FileHash $zipPath -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($expected.ToLowerInvariant() -ne $actual) {
        throw "Checksum verification failed"
    }

    Expand-Archive $zipPath -DestinationPath $tempDir -Force
    New-Item -ItemType Directory -Force $installDir | Out-Null
    Copy-Item (Join-Path $tempDir "tint.exe") (Join-Path $installDir "tint.exe") -Force
    Write-Host "Installed Tint $tag to $(Join-Path $installDir 'tint.exe')"
    Write-Host "Add $installDir to your PATH if it is not already there."
}
finally {
    Remove-Item $tempDir -Recurse -Force -ErrorAction SilentlyContinue
}
