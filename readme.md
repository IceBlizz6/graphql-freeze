# GraphQL Freeze
Provides a typesafe GraphQL client for TypeScript.

## How it works
Generates type info from your schema file or from introspection query to your endpoint.
Also generates a client you can customize yourself.
Consists of 2 parts: The codegen tool and the runtime for encoding/decoding.

## How it looks in your codebase
```typescript
const response = await query({
    getUser: [
        {
            id: 42
        },
        {
            name: 1,
            email: 1
        }
    ]
})
console.log(response.getUser.name)
```

```typescript
import { query } from "@/client"

const user = await queryAs({
    getUser: [
        {
            id: 42
        },
        {
            name: 1,
            email: 1
        }
    ],
    resp => resp.getUser
})
console.log(user.name)
```

## Setup guide
### Add dependencies
Exclude graphql-ws if you don't need GraphQL subscription support

Option 1. Add dependencies to package.json
```json
{
    "dependencies": {
        "graphql": "16.9.0",
        "graphql-ws": "5.16.0",
        "graphql-freeze": "0.1.3"
    },
    "dev-dependencies": {
        "graphql-freeze-codegen": "0.1.6"
    }
}
```
Option 2. npm cli
`npm install graphql graphql-ws graphql-freeze graphql-freeze-codegen`

### Generate the client
Option 1. Endpoint
`graphql-freeze -u http://localhost:8080/graphql -o src/gql`

Option 2. Schema file
`graphql-freeze -f resources/schema.graphql -o src/gql`

Option 3. Configuration file
graphql-freeze.json (in your root project folder)
```json
{
    "profiles": {
        "default": {
            "method": "endpoint",
            "url": "http://localhost:8080/graphql"
        }
    },
    "outputDirectory": "src/client"
}
```

We recommend configuration file as it gives a larger degree of customization.

## Setting up the generated client
3 files will be generated in output directory
* index.ts - Starter template for your GraphQL client, update this to better fit your project.
* schema.ts - schema types (will be overwritten on subsequent runs)
* codec.ts - encode/decode requests (will be overwritten on subsequent runs)

### Customize your index.ts (client code)

1. **Scalars**
Client will generate setup for built-in scalars but you will need to add in any other custom scalars defined in your schema

Example
```typescript
const appScalars = createScalars({
    ...,
    // string = type received in json from server
    // Moment = type handled locally in client
    LocalDate: scalar<string, Moment>({
        decode: (stringFromJsonResponse) => moment(stringFromJsonResponse),
        encode: (momentToBeSent) => momentToBeSent.format("yyyy-MM-DD")
    })
})
```

2. **Updating GraphQL endpoint**
Update endpoint if nescessary, find function named `sendRequest` in index.ts
```typescript
async function sendRequest(request: EncodedRequest): Promise<ExecutionResult> {
	const response = await fetch(
		"/graphql", // <-- Update the url if nescessary
		...
```
3. **Updating GraphQL subscription endpoint (or remove if not needed)**
Update endpoint if nescessary, find function named `subscription` in index.ts
```typescript
export async function subscription<T>(
	request: Exact<T, RequestType<"Subscription">>,
	onReceived: (_: OutputType<"Subscription", T>) => void
): Promise<void> {
	const client = createClient({
		url: "/graphql-subscription", // <-- Update the url if nescessary
		webSocketImpl: WebSocket
	})
```

## Configuration
### CLI
| Name | Type |Description | Default
| ------ | ------ | ----- | ----- |
| config (c) | string | Path to configuration file, relative to working directory | graphql-freeze.json
| profile (p) | string | Profile used from configuration file | default
| url (u) | string | Endpoint used with introspection query to extract types, overrides config file |
| file (f) | string | Path to graphql schema file, overrides config file |
| output (o) | string | Path to output directory relative to working directory, will create if not exists |
| errdump (e) | boolean | Prints content of endpoint response or file content to stderr if parsing fails, useful for troubleshooting | false
| help (h) | boolean | Print help message | false

All arguments are optional and configuration file is not required.
But will fail if no method to extract types has been provided.
Use either cli args or configuration, or combine both.
cli args will always override configuration file.

### Configuration file
| Name | Type | Description | Default
| ------ | ------ | ----- | ------ |
|profiles|Object|Profiles indexed by name
|outputDirectory|string|Path to output directory, relative to working directory
|lineBreak|string|Line break used in generated files|\r\n for windows, otherwise \n
|indent|string|Indent used in generated files| 4 spaces
|runtime|string|Runtime package included in imports for generated files|graphql-freeze

Profile options
1. From endpoint
```json
{
    "method": "endpoint",
    "url": "http://example.com/graphql"
}
```

2. From file
```json
{
    "method": "file",
    "path": "resources/schema.graphql"
}
```

3. Pipe introspection response into executable
```json
{
    "method": "pipeIntrospection"
}
```

3. Pipe graphql schema content into executable
```json
{
    "method": "pipeSdl"
}
```

## Advanced examples

### Multiple profiles in config file
graphql-freeze.json
```json
{
    "profiles": {
        "dev": {
            "method": "endpoint",
            "url": "http://localhost:8080/graphql"
        },
        "prod": {
            "method": "pipeSdl"
        }
    }
}
```
`./extract_sdl.sh | npm graphql-freeze`

### Extract type from query
```typescript
import { qSelect, OutputType } from "@/client"

const userRequest = qSelect("User", {
    id: 1,
    name: 1,
    hobbies: {
        name: 1
    }
})

type User = OutputType<"User", typeof userRequest>

function requestUser(id: number): Promise<User> {
    return queryAs(
        {
            getUser: [
                {
                    id: id
                },
                userRequest
            ]
        },
        resp => resp.getUser
    )
}
```

## How it works and usage

Codegen is written in rust for stability and performance

Type information from GraphQL is stored in typescript files inside the output directory.
If the schema changes then call graphql-freeze again to regenerate schema.ts and codec.ts
index.ts will not be overwritten, only created if it does not already exist.
index.ts is meant to be edited by you, to make it fit whatever your project needs.

TypeScript utility types are used in the background to resolve your requests into TypeScript types.

## Maintainer notes
Future development may include
* GraphQL INTERFACE support
* GraphQL UNION support
* Toggle whether TS null/undefined should be passed as GraphQL null in the request (useful for niche cases)
* Toggle whether TS null is allowed in addition to TS undefined
* Decode GraphQL null as:
    - TS null
    - TS undefined (explicitly assigned)
    - Excluded from object

Let us know if you need one of these in your project
