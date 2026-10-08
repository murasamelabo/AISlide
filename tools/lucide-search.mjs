function words(value) {
  return value
    .replace(/([A-Z])([A-Z][a-z])/g, '$1 $2')
    .replace(/([a-z0-9])([A-Z])/g, '$1 $2')
    .toLowerCase()
    .match(/[\p{L}\p{N}]+/gu) ?? [];
}

export function searchLucideIcons(entries, query) {
  const terms = words(query ?? '');
  const normalizedQuery = terms.join(' ');
  const ranked = [];
  for (const entry of entries) {
    const [name, metadata] = entry;
    let rank = 0;
    if (terms.length) {
      const nameWords = words(name);
      const normalizedName = nameWords.join(' ');
      if (normalizedName === normalizedQuery) {
        rank = 0;
      } else if (terms.every((term) => nameWords.includes(term))) {
        rank = 1;
      } else if (normalizedName.startsWith(normalizedQuery)) {
        rank = 2;
      } else {
        const allWords = [...nameWords, ...metadata.tags.flatMap(words), ...metadata.categories.flatMap(words)];
        if (terms.every((term) => allWords.includes(term))) {
          rank = 3;
        } else if (terms.every((term) => allWords.some((word) => word.startsWith(term)))) {
          rank = 4;
        } else {
          continue;
        }
      }
    }
    ranked.push({ entry, rank });
  }
  ranked.sort((left, right) => left.rank - right.rank
    || (left.entry[0] < right.entry[0] ? -1 : left.entry[0] > right.entry[0] ? 1 : 0));
  return ranked.map(({ entry }) => entry);
}

export function resolveLucideIcon(moduleExports, name, canonicalIcons = moduleExports.icons) {
  if (!canonicalIcons) return undefined;
  if (Object.hasOwn(canonicalIcons, name)) return { name, component: canonicalIcons[name] };
  if (!Object.hasOwn(moduleExports, name)) return undefined;
  const component = moduleExports[name];
  if (component === null || (typeof component !== 'object' && typeof component !== 'function')) return undefined;
  const canonical = Object.entries(canonicalIcons).find(([, candidate]) => candidate === component);
  return canonical ? { name: canonical[0], component } : undefined;
}