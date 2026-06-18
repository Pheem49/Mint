export function evaluateArithmetic(source: string): number {
  const tokens = source.replace(/\s+/g, '')
  let index = 0

  function parseExpression(): number {
    let value = parseTerm()
    while (tokens[index] === '+' || tokens[index] === '-') {
      const operator = tokens[index++]
      const right = parseTerm()
      value = operator === '+' ? value + right : value - right
    }
    return value
  }

  function parseTerm(): number {
    let value = parseFactor()
    while (tokens[index] === '*' || tokens[index] === '/') {
      const operator = tokens[index++]
      const right = parseFactor()
      value = operator === '*' ? value * right : value / right
    }
    return value
  }

  function parseFactor(): number {
    if (tokens[index] === '+' || tokens[index] === '-') {
      return tokens[index++] === '-' ? -parseFactor() : parseFactor()
    }
    if (tokens[index] === '(') {
      index += 1
      const value = parseExpression()
      if (tokens[index++] !== ')') throw new Error('missing closing parenthesis')
      return value
    }
    const match = tokens.slice(index).match(/^(?:\d+(?:\.\d*)?|\.\d+)/)
    if (!match) throw new Error('expected a number')
    index += match[0].length
    return Number(match[0])
  }

  if (!tokens) throw new Error('expression is empty')
  const result = parseExpression()
  if (index !== tokens.length || !Number.isFinite(result)) {
    throw new Error('invalid arithmetic expression')
  }
  return result
}
