import { Popover as PopoverPrimitive } from "bits-ui";
import Root from "./popover.svelte";
import Content from "./popover-content.svelte";
import Trigger from "./popover-trigger.svelte";

const Close = PopoverPrimitive.Close;
const Portal = PopoverPrimitive.Portal;

export {
	Root,
	Content,
	Trigger,
	Close,
	Portal,
	//
	Root as Popover,
	Content as PopoverContent,
	Trigger as PopoverTrigger,
	Close as PopoverClose,
	Portal as PopoverPortal,
};
