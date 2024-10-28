#!/usr/bin/env node
import fetch from "node-fetch"
import { fileURLToPath } from "url"
import { readFileSync, createWriteStream, chmodSync, mkdirSync, MakeDirectoryOptions, existsSync } from "fs"
import { dirname, join } from "path"

const arch = process.arch
const platform = process.platform

const installScriptLocation = fileURLToPath(import.meta.url)
const packageDirectory = dirname(installScriptLocation)
const packageFilePath = join(packageDirectory, "package.json")
const packageInfo = JSON.parse(readFileSync(packageFilePath, "utf8"))
const version = packageInfo.version

function getExecutableName(): string {
    switch (arch) {
        case "x64": {
            switch (platform) {
                case "win32": return "x86_64-pc-windows-msvc"
                case "linux": return "x86_64-unknown-linux-gnu"
                case "darwin": return "x86_64-apple-darwin"
                default: unsupported()
            }
        }
        case "arm64": {
            switch (platform) {
                case "win32": return "aarch64-pc-windows-msvc"
                case "linux": return "aarch64-unknown-linux-gnu"
                case "darwin": return "aarch64-apple-darwin"
                default: unsupported()
            }
        }
        default: unsupported()
    }
}

function unsupported(): never {
    throw new Error(`Unsupported platform: ${platform} or architecture: ${arch}`)
}

function getBinaryDestination() {
    const base = join(packageDirectory, "bin", "graphql-freeze")
    if (platform === "win32") {
        return `${base}.exe`
    } else {
        return base
    }
}

const fileName = getExecutableName()
const url = `https://github.com/IceBlizz6/graphql-freeze/releases/download/v${version}/${fileName}`
const response = await fetch(url)
const binaryDestination = getBinaryDestination()

if (response.ok) {
    const fileStream = createWriteStream(binaryDestination)
    if (response.body === null) {
        throw new Error(`Empty response from ${url}`)
    } else {
        const binDirectory = join(packageDirectory, "bin")
        if (!existsSync(binDirectory)) {
            mkdirSync(binDirectory)
        }
        const stream = response.body.pipe(fileStream)
        stream.on("finish", () => {
            chmodSync(binaryDestination, 0o755)
        })
    }
} else {
    throw new Error(`Error ${response.status}: ${response.statusText} for URL ${url}`)
}
