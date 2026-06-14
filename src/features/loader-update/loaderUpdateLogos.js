import fabricLogo from '../../assets/fabric-logo.png';
import neoforgeLogo from '../../assets/neoforge-logo.png';

const LOADER_LOGOS = {
  neoforge: { src: neoforgeLogo, alt: 'NeoForge', label: 'NeoForge' },
  fabric: { src: fabricLogo, alt: 'Fabric', label: 'Fabric' }
};

export function loaderLogo(loader) {
  return LOADER_LOGOS[loader] ?? LOADER_LOGOS.neoforge;
}

export function loaderLabel(loader) {
  return loaderLogo(loader).label;
}
