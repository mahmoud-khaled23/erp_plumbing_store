import { useEffect, useState } from 'react';
import { assetUrl } from '../api/client';
import { t } from '../i18n';
export function ProductImage({ path }: { path: string }) {
  const [url, setUrl] = useState('');
  useEffect(() => {
    let live = true;
    assetUrl(path)
      .then((u) => {
        if (live) setUrl(u);
      })
      .catch(() => {});
    return () => {
      live = false;
    };
  }, [path]);
  return url ? (
    <img
      src={url}
      alt={t('image')}
      className="h-24 w-24 rounded-xl border border-slate-200 object-contain"
    />
  ) : null;
}
