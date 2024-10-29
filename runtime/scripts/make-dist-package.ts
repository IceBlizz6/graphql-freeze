import { dirname, join } from "path"
import { fileURLToPath } from "url"
import { readFileSync, writeFileSync } from "fs"

const scriptLocation = fileURLToPath(import.meta.url)
const scriptsFolder = dirname(scriptLocation)
const projectFolder = dirname(scriptsFolder)
const originalPackageContent = readFileSync(join(projectFolder, "package.json"), "utf-8")
const originalPackage = JSON.parse(originalPackageContent)

const packageInfo = {
	"name": originalPackage.name,
	"version": originalPackage.version,
	"type": "module"
}

writeFileSync(
	join(projectFolder, "dist", "package.json"),
	JSON.stringify(packageInfo, null, 2)
)
