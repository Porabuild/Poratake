const NAMED_ENTITIES: Record<string, string> = {
  amp: '&',
  lt: '<',
  gt: '>',
  quot: '"',
  apos: "'",
  nbsp: ' ',
  mdash: '—',
  ndash: '–',
  hellip: '…',
  copy: '©',
};

function decodeCodePoint(match: string, value: string, radix: number): string {
  const codePoint = Number.parseInt(value, radix);
  if (
    !Number.isSafeInteger(codePoint) ||
    codePoint === 0 ||
    codePoint > 0x10ffff ||
    (codePoint >= 0xd800 && codePoint <= 0xdfff)
  ) {
    return match;
  }
  return String.fromCodePoint(codePoint);
}

function decodeEntities(text: string): string {
  return text
    .replace(/&#x([0-9a-f]+);/gi, (match, hex) =>
      decodeCodePoint(match, hex, 16)
    )
    .replace(/&#(\d+);/g, (match, dec) => decodeCodePoint(match, dec, 10))
    .replace(
      /&([a-z]+);/gi,
      (match, name) => NAMED_ENTITIES[name.toLowerCase()] ?? match
    );
}

export function releaseNotesToText(notes: string): string {
  const withoutBlocks = notes
    .replace(/<(script|style)[^>]*>[\s\S]*?<\/\s*\1\s*>/gi, '')
    .replace(/<br[^>]*>/gi, '\n')
    .replace(/<li[^>]*>/gi, '\n- ')
    .replace(/<\/(h[1-6]|p|div|ul|ol|li|blockquote|pre|table|tr)[^>]*>/gi, '\n')
    .replace(/<[^>]+>/g, '');

  return decodeEntities(withoutBlocks)
    .split('\n')
    .map(line => line.replace(/\s+/g, ' ').trim())
    .join('\n')
    .replace(/\n{3,}/g, '\n\n')
    .trim();
}
