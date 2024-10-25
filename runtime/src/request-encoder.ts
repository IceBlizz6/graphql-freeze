import { Codec, CodecField, Encoder } from "."

export function encodeRequest(operationType: string, request: Object, codec: Codec): EncodedRequest {
    const encoding = new GraphQLRequest()
    const encodedObject = encoding.encodeObject(request, codec)
    const encodedRequest = `${operationType}${encoding.variablesSupplyList()} ${encodedObject}`
    return {
        query: encodedRequest,
        variables: encoding.variablesJsonObject()
    }
}

export interface EncodedRequest {
    query: string
    variables: Record<string, unknown> | undefined
}

interface Variable {
    type: string
    name: string
    value: unknown
}

class GraphQLRequest {
    private readonly variables: Variable[] = []
    private variableCounter = 1

    public variablesSupplyList(): string {
        if (this.variables.length === 0) {
            return ""
        } else {
            const segments = this.variables
                .map(e => {
                    return `\$${e.name}: ${e.type}`
                })
                .join(", ")
            return `(${segments})`
        }
    }

    public variablesJsonObject(): Record<string, unknown> | undefined {
        if (this.variables.length === 0) {
            return undefined
        } else {
            return this.variables
                .map(e => {
                    return {
                        [e.name]: e.value
                    }
                })
                .reduce((a, b) => Object.assign(a, b), {})
        }
    }

    public encodeObject(obj: unknown, codec: Codec): string {
        if (obj !== null && typeof obj === "object") {
            const request = Object.entries(obj)
                .map(([fieldName, fieldValue]) => {
                    const encoderField = codec[fieldName]
                    if (encoderField === undefined) {
                        throw new Error(`Encoder has no field for ${fieldName} on ${codec}`)
                    } else {
                        return this.encodeField(fieldName, fieldValue, encoderField)
                    }
                })
                .join(" ")
            return `{ ${request} }`
        } else {
            throw new Error(`Expected object, but instead got ${obj}`)
        }
    }
    
    private encodeField(field: string, value: unknown, codec: CodecField): string {
        if (typeof value === "number") {
            return field
        } else if (Array.isArray(value)) {
            return this.encodeFunction(field, value[0], value[1], codec)
        } else if (codec.codec === undefined) {
            throw new Error(`Missing encode prop for ${field} in ${codec}`)
        } else if (typeof value === "object") {
            return `${field} ${this.encodeObject(value, codec.codec())}`
        } else {
            throw new Error(`Expected number, array or object, but instead got ${value} for field ${field}`)
        }
    }
    
    private encodeFunction(field: string, params: Object, fieldRequest: unknown, encoder: CodecField): string {
        if (encoder.args === undefined) {
            throw new Error(`Missing args prop on ${encoder}`)
        } else {
            const encodeArgs = encoder.args
            const input = Object.entries(params)
                .filter(([_, argValue]) => argValue !== null && argValue !== undefined)
                .map(([argName, argValue]) => {
                    const encodeArg = encodeArgs[argName]
                    if (encodeArg === undefined) {
                        throw new Error(`No encoding found for argument ${argName}`)
                    } else {
                        const value = encodeArg.encode(argValue)
                        const variableName = this.newVariableName()
                        this.variables.push({
                            name: variableName,
                            type: encodeArg.type,
                            value: value
                        })
                        return `${argName}: \$${variableName}`
                    }
                })
            if (typeof fieldRequest === "number") {
                if (input.length === 0) {
                    return field
                } else {
                    return `${field}(${input.join(", ")})`
                }
            } else if (typeof fieldRequest === "object") {
                if (encoder.codec === undefined) {
                    throw new Error(`Missing encode prop on ${encoder}`)
                } else {
                    const subRequest = this.encodeObject(fieldRequest, encoder.codec())
                    if (input.length === 0) {
                        return `${field} ${subRequest}`
                    } else {
                        return `${field}(${input.join(", ")}) ${subRequest}`
                    }
                }
            } else {
                throw new Error(`Field request for function expected 1 or object, instead got ${fieldRequest}`)
            }
        }
    }

    private newVariableName(): string {
        return `v${this.variableCounter++}`
    }
}
