import { BookOpen, Globe2, Monitor, Server, Wrench } from 'lucide-react';
import curseforgeIcon from '../assets/curseforge-icon.svg';
import externalIcon from '../assets/external-icon.svg';
import modrinthIcon from '../assets/modrinth-icon.svg';

export const sideOptions = [
  { id: 'client', icon: Monitor, label: 'Клиент', tone: 'client' },
  { id: 'universal', icon: Globe2, label: 'Универсальный', tone: 'universal' },
  { id: 'server', icon: Server, label: 'Сервер', tone: 'server' }
];

export const filters = [
  { id: 'all', label: 'Все' },
  ...sideOptions,
  { id: 'library', icon: BookOpen, label: 'Библиотека', tone: 'library' },
  { id: 'technical', icon: Wrench, label: 'Оптимизация', tone: 'technical' }
];

export const sourceIcons = {
  modrinth: { icon: modrinthIcon, label: 'Modrinth' },
  curseforge: { icon: curseforgeIcon, label: 'CurseForge' },
  index: { icon: externalIcon, label: 'Pack index' },
  manual: { icon: externalIcon, label: 'Сторонний мод' }
};

export function formatDate(value) {
  if (!value) return '';
  return new Intl.DateTimeFormat('ru', {
    day: '2-digit',
    month: 'short',
    hour: '2-digit',
    minute: '2-digit'
  }).format(new Date(value));
}

export function modByKey(mods) {
  return new Map(mods.map((mod) => [mod.key, mod]));
}
