import { renderTemplate, type TemplateData, type TemplateName } from "./templates.logic";
import { sendEmail } from "./transport";

/**
 * Sending half of the email layer. The markup lives in ./templates.logic,
 * which stays free of `$app/*` and the transport so the vitest suite (node
 * env, no SvelteKit plugins) can render and assert on real output.
 */

export { renderTemplate, type TemplateData, type TemplateName } from "./templates.logic";

/**
 * Send a templated email. Single entrypoint for every transactional message —
 * keeps subjects/copy in one file and rendering centralized.
 */
export async function sendTemplatedEmail<N extends TemplateName>(args: {
	to: string;
	template: N;
	data: TemplateData[N];
	replyTo?: string;
}): Promise<void> {
	const rendered = renderTemplate(args.template, args.data);
	await sendEmail({
		to: args.to,
		subject: rendered.subject,
		text: rendered.text,
		html: rendered.html,
		replyTo: args.replyTo,
	});
}
