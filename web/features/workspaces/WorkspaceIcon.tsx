import { Folder, Server } from "lucide-react";
import { useState } from "react";

export function WorkspaceIcon({
  className = "size-4 shrink-0 rounded object-cover",
  fallbackClassName = "size-4 shrink-0",
  isRemote = false,
  logoUrl,
}: {
  className?: string;
  fallbackClassName?: string;
  isRemote?: boolean;
  logoUrl: string | null | undefined;
}) {
  const [failedLogoUrl, setFailedLogoUrl] = useState<string | null>(null);
  const shouldShowLogo = Boolean(logoUrl && failedLogoUrl !== logoUrl);

  if (shouldShowLogo && logoUrl) {
    return (
      <img
        alt=""
        aria-hidden="true"
        className={className}
        onError={() => setFailedLogoUrl(logoUrl)}
        src={logoUrl}
      />
    );
  }

  const Icon = isRemote ? Server : Folder;
  return <Icon aria-hidden="true" className={fallbackClassName} />;
}
