import { type ClassValue, clsx } from "clsx";
import { twMerge } from "tailwind-merge";

/** shadcn-vue class-name merge helper. */
export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}
