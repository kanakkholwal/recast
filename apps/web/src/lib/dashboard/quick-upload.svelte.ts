class QuickUploadState {
	open = $state(false);

	show() {
		this.open = true;
	}

	hide() {
		this.open = false;
	}
}

export const quickUpload = new QuickUploadState();
