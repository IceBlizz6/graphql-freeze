set -e

pnpm build

cd dist
pnpm publish
