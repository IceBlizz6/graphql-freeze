export interface Scalar<Encoded, Decoded> {
    encode(value: Decoded): Encoded

    decode(value: Encoded): Decoded
}

export function scalar<Encoded, Decoded>(
	codec: {
		encode: (input: Decoded) => Encoded
		decode: (input: Encoded) => Decoded
	}
): Scalar<Encoded, Decoded> {
	return codec
}

export type Codec = Record<string, CodecField>

export type CodecField = {
	decode: (value: unknown) => unknown,
	codec?: () => Codec,
	args?: Record<string, {
		type: string,
		encode: (value: unknown) => unknown
	}>
}

export type Encoder = Record<string, (value: unknown) => unknown>

export function decodeObjectResponse(value: Object, decoder: Codec): Record<string, unknown> {
	return decodeObject(value, decoder)
}

export function decodeObject<Encoded>(value: Encoded, decoder: Codec): Record<string, unknown> {
    if (value != null && typeof value === "object") {
        return Object.entries(value)
            .map(([fieldName, fieldValue]) => {
                const decodeField = decoder[fieldName]?.decode
                if (decodeField === undefined) {
                    throw new Error(`Missing decoder for ${fieldName} on ${decoder}`)
                } else {
                    return {
                        [fieldName]: decodeField(fieldValue)
                    }
                }
            })
            .reduce((a, b) => Object.assign(a, b), {})
    } else {
        throw new Error(`Expected object, but instead got ${value}`)
    }
}

export function decodeList<Encoded, Decoded>(value: unknown, decode: (_: Encoded) => Decoded): Decoded[] {
    if (Array.isArray(value)) {
        return value.map(decode)
    } else {
        throw new Error(`Expected array, but instead got ${value}`)
    }
}

export function decodeNull<Encoded, Decoded>(value: Encoded | null, decode: (_: Encoded) => Decoded): Decoded | null {
    if (value === null || value === undefined) {
        return null
    } else {
        return decode(value)
    }
}

export function encodeNull<Encoded, Decoded>(value: Decoded, encode: (_: Decoded) => Encoded): Encoded | null {
    if (value === null || value === undefined) {
        return null
    } else {
        return encode(value)
    }
}

export function encodeList<Encoded, Decoded>(value: unknown, encode: (_: Decoded) => Encoded): Encoded[] {
    if (Array.isArray(value)) {
        return value.map(item => encode(item))
    } else {
        throw new Error(`Expected array, but instead got ${value}`)
    }
}

export function encodeObject(value: unknown, encoder: Encoder): { [name: string]: unknown } {
    if (value !== null && typeof value === "object") {
        return Object.entries(value)
            .map(([fieldName, fieldValue]) => {
                const encodeField = encoder[fieldName]
                if (encodeField === undefined) {
                    throw new Error(`No input encoder found for ${fieldName} on ${encoder}`)
                } else {
                    return {
                        [fieldName]: encodeField(fieldValue)
                    }
                }
            })
            .reduce((a, b) => Object.assign(a, b), {})
    } else {
        throw new Error(`Expected object, but instead got ${value}`)
    }
}

export type QScalar<T> = { scalar: T }

export type QObject<T> = { object: T }

export type QList<T> = { array: T }

export type QEnum<T> = { enum: T }

export type QNull<T> = { value: T }

export type QFun<Params, Fields> = { params: Params, fields: Fields }

export type Narrowable = | string | number | bigint | boolean

export type Exact<A, W> =
    W extends unknown ? (
        A extends W ? 
        A extends Narrowable ? 
        A: { [K in keyof A]: K extends keyof W ? Exact<A[K], W[K]> : never } : W
    ) : never

export type GraphQLNull = null

export type DecodeScalar<TScalars extends Record<string, Scalar<unknown, unknown>>, Key> = Key extends keyof TScalars ? ReturnType<TScalars[Key]["decode"]> : never

export type ObjectRequest<TSchema, TInputSchema, TScalars extends Record<string, Scalar<unknown, unknown>>, T> = Partial<{
    [F in keyof T]: FieldRequest<TSchema, TInputSchema, TScalars, T[F]>
}>

export type FieldRequest<TSchema, TInputSchema, TScalars extends Record<string, Scalar<unknown, unknown>>, T> =
	T extends QScalar<infer _> ? number :
	T extends QEnum<infer _> ? number :
	T extends QObject<infer GObject> ? ObjectRequest<TSchema, TInputSchema, TScalars, (GObject extends keyof TSchema ? TSchema[GObject] : never)> :
	T extends QList<infer A> ? FieldRequest<TSchema, TInputSchema, TScalars, A> :
	T extends QNull<infer U> ? FieldRequest<TSchema, TInputSchema, TScalars, U> :
	T extends QFun<infer Params, infer Fields> ? [InputObjectType<TInputSchema, TScalars, Params>, FieldRequest<TSchema, TInputSchema, TScalars, Fields>] :
	never

export type InputObjectType<TInputSchema, TScalars extends Record<string, Scalar<unknown, unknown>>, T> = {
	[F in keyof T]: InputFieldType<TInputSchema, TScalars, T[F]>
}

export type InputFieldType<TInputSchema, TScalars extends Record<string, Scalar<unknown, unknown>>, T> =
	T extends QScalar<infer ScalarType> ? DecodeScalar<TScalars, ScalarType> :
	T extends QEnum<infer U> ? U :
	T extends QObject<infer GObject> ? InputObjectType<TInputSchema, TScalars, (GObject extends keyof TInputSchema ? TInputSchema[GObject] : never)> :
	T extends QList<infer A> ? Array<InputFieldType<TInputSchema, TScalars, A>> :
	T extends QNull<infer U> ? (InputFieldType<TInputSchema, TScalars, U> | GraphQLNull) :
	never

export type OutputObjectType<TSchema, TScalars extends Record<string, Scalar<unknown, unknown>>, TRequest, TModel> = {
	[F in keyof TRequest]: F extends keyof TModel ? OutputFieldType<TSchema, TScalars, TRequest[F], TModel[F]> : never
}

export type OutputFieldType<TSchema, TScalars extends Record<string, Scalar<unknown, unknown>>, TRequest, TModel> =
	TRequest extends number ? (
        TModel extends QScalar<infer U> ? DecodeScalar<TScalars, U> :
        TModel extends QNull<infer U> ? (OutputFieldType<TSchema, TScalars, TRequest, U> | GraphQLNull) :
        TModel extends QList<infer U> ? Array<OutputFieldType<TSchema, TScalars, TRequest, U>> :
        TModel extends QEnum<infer U> ? U :
        never
    ) :
    TRequest extends Array<infer _> ? (
        TModel extends QFun<infer _, infer Return> ? OutputFieldType<TSchema, TScalars, TRequest[1], Return> : never
    ) :
    TRequest extends Object ? (
        TModel extends QObject<infer U> ? OutputObjectType<TSchema, TScalars, TRequest, (U extends keyof TSchema ? TSchema[U] : never)> :
        TModel extends QNull<infer U> ? (OutputFieldType<TSchema, TScalars, TRequest, U> | GraphQLNull) :
        TModel extends QList<infer U> ? Array<OutputFieldType<TSchema, TScalars, TRequest, U>> :
        never
    ) :
    never
