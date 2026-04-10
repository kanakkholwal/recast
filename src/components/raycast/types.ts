import type { Component } from "svelte";

export type RaycastIcon = Component<{ size?: number | string; class?: string }>;

export interface RaycastAccessory {
	text?: string;
	icon?: RaycastIcon;
	tooltip?: string;
	variant?: "default" | "success" | "warning" | "destructive" | "info";
}

export interface RaycastAction {
	id: string;
	label: string;
	icon?: RaycastIcon;
	shortcut?: string;
	variant?: "default" | "destructive";
	onAction: () => void | Promise<void>;
}

export interface RaycastListItem {
	id: string;
	title: string;
	subtitle?: string;
	icon?: RaycastIcon;
	iconImage?: string;
	iconClass?: string;
	keywords?: string[];
	accessories?: RaycastAccessory[];
	section?: string;
	actions?: RaycastAction[];
	onSelect?: () => void | Promise<void>;
}

export interface RaycastSection {
	title: string;
	items: RaycastListItem[];
}
