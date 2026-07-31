import { withBase } from "@rspress/core/runtime";
import type { AnchorHTMLAttributes } from "react";
import { canonicalSitePath } from "../../lib/i18n";

interface SiteLinkProps extends AnchorHTMLAttributes<HTMLAnchorElement> {
  href: string;
}

export function SiteLink({ href, ...props }: SiteLinkProps) {
  const resolvedHref = href.startsWith("/")
    ? withBase(canonicalSitePath(href))
    : href;
  return <a {...props} href={resolvedHref} />;
}
