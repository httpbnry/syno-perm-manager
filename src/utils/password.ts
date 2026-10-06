/** Uniform sampling from the OS-backed Web Crypto generator. */
export function generatePassword(length = 16): string {
  const chars = 'abcdefghijkmnopqrstuvwxyzABCDEFGHJKLMNPQRSTUVWXYZ23456789!@#%'
  if (!Number.isInteger(length) || length < 12 || length > 128) throw new Error('Longitud de contraseña inválida')
  const limit = 256 - (256 % chars.length)
  let result = ''
  while (result.length < length) {
    const bytes = crypto.getRandomValues(new Uint8Array(length * 2))
    for (const byte of bytes) {
      if (byte < limit) result += chars[byte % chars.length]
      if (result.length === length) return result
    }
  }
  return result
}
