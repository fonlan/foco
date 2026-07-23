import { Trash2 } from "lucide-react";

import type { PendingDeleteChat } from "../../api/types";
import { useI18n } from "../../shared/i18n";
import { Button, Modal } from "../../shared/ui";

export function DeleteChatDialog({
  chat,
  onClose,
  onConfirm,
}: {
  chat: PendingDeleteChat;
  onClose: () => void;
  onConfirm: () => void;
}) {
  const { t } = useI18n();

  return (
    <Modal.Backdrop isDismissable isOpen onOpenChange={(open) => !open && onClose()}>
      <Modal.Container placement="center" size="sm">
        <Modal.Dialog aria-label={t("Delete this chat?")}>
          <Modal.CloseTrigger />
          <Modal.Header>
            <Modal.Icon className="bg-danger-soft text-danger-soft-foreground">
              <Trash2 aria-hidden="true" className="size-5" />
            </Modal.Icon>
            <Modal.Heading>{t("Delete this chat?")}</Modal.Heading>
          </Modal.Header>
          <Modal.Body className="space-y-3">
            <div>
              <p className="text-sm font-medium text-foreground">{chat.title}</p>
              <p className="mt-1 text-xs font-medium text-muted">
                {chat.workspaceName}
              </p>
            </div>
            <p className="text-sm leading-6 text-muted">
              {t("This will delete the saved chat history.")}
            </p>
          </Modal.Body>
          <Modal.Footer>
            <Button
              aria-label={t("Cancel chat deletion")}
              slot="close"
              variant="tertiary"
              onPress={onClose}
            >
              {t("Cancel")}
            </Button>
            <Button
              aria-label={t("Confirm delete chat")}
              variant="danger"
              onPress={onConfirm}
            >
              <Trash2 aria-hidden="true" className="size-4" />
              {t("Delete chat")}
            </Button>
          </Modal.Footer>
        </Modal.Dialog>
      </Modal.Container>
    </Modal.Backdrop>
  );
}
