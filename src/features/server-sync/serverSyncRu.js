function pluralForm(count, one, few, many) {
  const n100 = count % 100;
  const n10 = count % 10;
  if (n100 >= 11 && n100 <= 14) return many;
  if (n10 === 1) return one;
  if (n10 >= 2 && n10 <= 4) return few;
  return many;
}

export function formatFiles(count) {
  return `${count} ${pluralForm(count, 'файл', 'файла', 'файлов')}`;
}
