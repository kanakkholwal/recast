import { Mail } from "@recast/icons";
import { FacebookBrand, LinkedinBrand, XBrand } from '@recast/ui/brand-icons';

type ShareData = {
  title?: string;
  text?: string;
  url?: string;
  image?: string;
};

export function useShare(getData: () => ShareData) {
  // `navigator.share` isn't reactive, so read it once rather than via `$effect`.
  const isNativeShareSupported =
    typeof navigator !== "undefined" && typeof navigator.share === "function";

  const share = async () => {

    const data = getData(); 
    
    if (navigator && navigator.share) {
      try {
        await navigator.share({
          title: data.title,
          text: data.text,
          url: data.url,
        });
      } catch (error) {
        console.error("Failed to share content", error);
      }
    } else {
      console.warn("Web Share API not supported");
    }
  };


  let socials = $derived.by(() => {
    const data = getData();
    return [
      {
        name: "facebook",
        url: `https://www.facebook.com/sharer/sharer.php?u=${encodeURIComponent(data.url || '')}`,
        icon: FacebookBrand,
      },
      {
        name: "twitter",
        url: `https://twitter.com/intent/tweet?url=${encodeURIComponent(data.url || '')}&text=${encodeURIComponent(data.title || '')}`,
        icon: XBrand,
      },
      {
        name: "linkedin",
        url: `https://www.linkedin.com/shareArticle?mini=true&url=${encodeURIComponent(data.url || '')}&title=${encodeURIComponent(data.title || '')}`,
        icon: LinkedinBrand,
      },
      
      {
        name: "email",
        url: `mailto:?subject=${encodeURIComponent(data.title || '')}&body=${encodeURIComponent(data.text || '')}: ${encodeURIComponent(data.url || '')}`,
        icon: Mail,
      },
    ];
  });

  return {
    share,
    isNativeShareSupported,
    get socials() { return socials } // Return the reactive derived value
  };
}
