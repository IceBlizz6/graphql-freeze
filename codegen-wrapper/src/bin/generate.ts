#!/usr/bin/env node
import { fileURLToPath } from "url"
import { dirname, join } from "path"
import { execFile } from 'child_process'

const installScriptLocation = fileURLToPath(import.meta.url)
const binFolder = dirname(installScriptLocation)
const platform = process.platform
const binaryPath = getBinaryPath()
executeBinary()

function getBinaryPath() {
	if (platform === "win32") {
		return join(binFolder, "graphql-freeze.exe")
	} else {
		return join(binFolder, "graphql-freeze")
	}
}

function executeBinary() {
	execFile(binaryPath, (error, stdout, stderr) => {
		if (error) {
			console.error(`Error executing binary: ${error.message}`)
		}
		if (stderr) {
			console.error(stderr)
		}
		console.log(stdout)
	})
}
