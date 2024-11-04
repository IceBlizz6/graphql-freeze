import { dirname, join } from "path"
import { fileURLToPath } from "url"
import { readFileSync, writeFileSync } from "fs"
import { parse as parseToml } from "toml"

const scriptLocation = fileURLToPath(import.meta.url)
const scriptsFolder = dirname(scriptLocation)
const projectFolder = dirname(scriptsFolder)
const originalPackageContent = readFileSync(join(projectFolder, "package.json"), "utf-8")
const originalPackage = JSON.parse(originalPackageContent)

const rootFolder = dirname(projectFolder)
const codegenTomlFile = join(rootFolder, "codegen", "Cargo.toml")
const codegenTomlContent = readFileSync(codegenTomlFile, "utf8")
const codegenToml = parseToml(codegenTomlContent)
const codegenVersion = codegenToml.package.version

if (typeof codegenVersion !== "string") {
	throw new Error("Unable to extract codegen version")
}

const packageInfo = {
	"name": originalPackage.name,
	"version": originalPackage.version,
	"type": "module",
	"bin": {
		"graphql-freeze": "./bin/graphql-freeze"
	},
	"scripts": {
		"postinstall": "./install.js"
	},
	"publishConfig": {
		"executableFiles": [
			"./install.js"
		],
	},
	"binary-version": codegenVersion
}

writeFileSync(
	join(projectFolder, "dist", "package.json"),
	JSON.stringify(packageInfo, null, 2)
)
