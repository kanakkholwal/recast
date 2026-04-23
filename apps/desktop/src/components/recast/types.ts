import type { Component } from "svelte";

export type RecastIcon = Component<{ size?: number | string; class?: string }>;

export interface RecastAccessory {
	text?: string;
	icon?: RecastIcon;
	tooltip?: string;
	variant?: "default" | "success" | "warning" | "destructive" | "info";
}

export interface RecastAction {
	id: string;
	label: string;
	icon?: RecastIcon;
	shortcut?: string;
	variant?: "default" | "destructive";
	onAction: () => void | Promise<void>;
}

export type RecastLayout = "card" | "row";

export interface RecastListItem {
	id: string;
	title: string;
	subtitle?: string;
	icon?: RecastIcon;
	iconImage?: string;
	iconClass?: string;
	keywords?: string[];
	accessories?: RecastAccessory[];
	section?: string;
	layout?: RecastLayout;
	actions?: RecastAction[];
	onSelect?: () => void | Promise<void>;
}

export interface RecastSection {
	title: string;
	items: RecastListItem[];
}
