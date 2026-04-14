import type { PageLoad } from './$types';


export const ssr = false;

export const load: PageLoad = async ({ fetch, setHeaders }) => {
  // Cache this response in the browser/CDN for 1 hour to prevent GitHub API rate limits
  setHeaders({
    'Cache-Control': 'public, max-age=3600'
  });

  try {
    const response = await fetch('https://api.github.com/repos/kanakkholwal/recast/releases/latest');
    
    if (!response.ok) throw new Error('Failed to fetch release');
    
    const release = await response.json();
    const version: string = release.tag_name; // e.g., "v0.x.x-preview"
    const assets: { name: string; browser_download_url: string }[] = release.assets;

    // Windows: prefer .msi, fall back to .exe (skip .sig files)
    const windowsMsi = assets.find(a => a.name.endsWith('.msi') && !a.name.endsWith('.sig'));
    const windowsExe = assets.find(a => a.name.endsWith('.exe') && !a.name.endsWith('.sig'));

    // macOS: .dmg files
    const macosIntel = assets.find(a => a.name.includes('x64') && a.name.endsWith('.dmg') && !a.name.endsWith('.sig'));
    const macosAppleSilicon = assets.find(a => a.name.includes('aarch64') && a.name.endsWith('.dmg') && !a.name.endsWith('.sig'));

    // Linux
    const linuxAppImage = assets.find(a => a.name.endsWith('.AppImage') && !a.name.endsWith('.sig'));
    const linuxDeb      = assets.find(a => a.name.endsWith('.deb')      && !a.name.endsWith('.sig'));
    const linuxRpm      = assets.find(a => a.name.endsWith('.rpm')      && !a.name.endsWith('.sig'));

    return {
      version,
      downloads: {
        windowsMsi:           windowsMsi?.browser_download_url       ?? null,
        windowsExe:           windowsExe?.browser_download_url       ?? null,
        macosIntel:           macosIntel?.browser_download_url       ?? null,
        macosAppleSilicon:    macosAppleSilicon?.browser_download_url ?? null,
        linuxAppImage:        linuxAppImage?.browser_download_url     ?? null,
        linuxDeb:             linuxDeb?.browser_download_url         ?? null,
        linuxRpm:             linuxRpm?.browser_download_url         ?? null,
      }
    };
  } catch (error) {
    // Fallback if the API fails or no release has been published yet
    return {
      version: 'Latest',
      downloads: {
        windowsMsi: null, windowsExe: null,
        macosIntel: null, macosAppleSilicon: null,
        linuxAppImage: null, linuxDeb: null, linuxRpm: null,
      }
    };
  }
};