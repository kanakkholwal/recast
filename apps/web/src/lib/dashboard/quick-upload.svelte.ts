class QuickUploadState {
	open = $state(false);
	/**
	 * A file handed straight to the dialog (e.g. from a drag-and-drop onto the
	 * library). When set, the dialog auto-starts its upload on open so the drop
	 * flows into the same share journey as the button.
	 */
	pendingFile = $state<File | null>(null);

	show(file?: File) {
		this.pendingFile = file ?? null;
		this.open = true;
	}

	hide() {
		this.open = false;
		this.pendingFile = null;
	}
}

export const quickUpload = new QuickUploadState();
