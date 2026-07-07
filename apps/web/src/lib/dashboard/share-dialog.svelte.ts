class ShareDialogState {
	open = $state(false);
	/** The recast a share link is being configured for. */
	recastId = $state<string | null>(null);

	show(recastId: string) {
		this.recastId = recastId;
		this.open = true;
	}

	hide() {
		this.open = false;
		this.recastId = null;
	}
}

export const shareDialog = new ShareDialogState();
