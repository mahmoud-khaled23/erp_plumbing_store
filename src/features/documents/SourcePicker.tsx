import { useState } from 'react';
import { api, s, type DocumentDetail } from '../../api/client';
import { useQuery, useTask } from '../../lib/hooks';
import { Button, ErrorState, Loading, Modal } from '../../components/ui';
import { DataTable, Pager } from '../../components/Table';
import { t } from '../../i18n';
export function SourcePicker({
  kind,
  onSelect,
  onClose,
}: {
  kind: 'purchase_order' | 'goods_receipt' | 'sale' | 'purchase';
  onSelect: (document: DocumentDetail) => void | Promise<void>;
  onClose: () => void;
}) {
  const [cursors, setCursors] = useState(['0']);
  const result = useQuery({
    type: 'documents',
    kind,
    status: kind === 'purchase_order' ? 'approved' : 'posted',
    after_id: cursors.at(-1)!,
    limit: 50,
  });
  const task = useTask();
  return (
    <Modal title="selectSource" onClose={onClose} wide busy={task.busy}>
      {result.error ? (
        <ErrorState error={result.error} />
      ) : result.loading ? (
        <Loading />
      ) : (
        <DataTable
          rows={(result.data ?? []).filter(
            (r) => !r.reversal_number && (kind !== 'purchase_order' || r.progress !== 'closed'),
          )}
          columns={['number', 'document_date', 'party_name_snapshot', 'total_minor', 'status']}
          actions={(r) => (
            <Button
              busy={task.busy}
              onClick={() =>
                task.run(async () => {
                  await onSelect(await api.query({ type: 'document', id: s(r, 'id') }));
                  onClose();
                })
              }
            >
              {t('useSource')}
            </Button>
          )}
        />
      )}
      <Pager
        page={cursors.length - 1}
        canNext={result.data?.length === 50}
        onPrevious={() => setCursors((x) => x.slice(0, -1))}
        onNext={() => setCursors((x) => [...x, s(result.data?.at(-1), 'id')])}
      />
    </Modal>
  );
}
