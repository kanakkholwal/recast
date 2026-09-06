// SSR is on so the signed-in guard redirects before paint; with it off the login form flashed on every visit. No `load` here: a universal one would replace the server data and drop `socialProviders`.
export const prerender = false;
