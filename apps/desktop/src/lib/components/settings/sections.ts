import type { Component } from 'svelte';
import AiSection from '#lib/components/ai/AiSection.svelte';
import AboutSection from './AboutSection.svelte';
import AppearanceSection from './AppearanceSection.svelte';
import EditorSection from './EditorSection.svelte';
import ShortcutsSection from './ShortcutsSection.svelte';
import WorkspaceSection from './WorkspaceSection.svelte';

/** The sections of the Settings page, in order; each renders under its title. */
interface SettingsSection {
	id: string;
	title: string;
	component: Component;
}

export const settingsSections: SettingsSection[] = [
	{ id: 'appearance', title: 'Appearance', component: AppearanceSection },
	{ id: 'editor', title: 'Editor', component: EditorSection },
	{ id: 'workspace', title: 'Workspace', component: WorkspaceSection },
	{ id: 'ai', title: 'AI', component: AiSection },
	{ id: 'shortcuts', title: 'Keyboard shortcuts', component: ShortcutsSection },
	{ id: 'about', title: 'About', component: AboutSection }
];
