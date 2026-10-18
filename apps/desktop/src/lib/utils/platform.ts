/** macOS gets the overlay title bar (traffic lights over the sidebar) and ⌘ shortcuts. */
export const isMac = typeof navigator !== 'undefined' && navigator.userAgent.includes('Mac');

/** The file manager's name only fits macOS; Windows and Linux get a neutral label. */
export const revealLabel = isMac ? 'Reveal in Finder' : 'Show in folder';
