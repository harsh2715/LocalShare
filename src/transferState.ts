export type Transfer = {
  peer_id: string;
  id: string;
  name: string;
  size: number;
  transferred: number;
  state: string;
  direction: string;
  path?: string;
  files?: string[];
  directories?: string[];
};

export type TransferPayload = Partial<Transfer> & { id: string; message?: string };

export function mergeTransfer(
  old: Transfer[],
  payload: TransferPayload,
  fallbackState: string,
): Transfer[] {
  const previous = old.find((item) => item.id === payload.id);
  const next: Transfer = {
    peer_id: payload.peer_id ?? previous?.peer_id ?? '',
    id: payload.id,
    name: payload.name ?? previous?.name ?? 'Transfer',
    size: payload.size ?? previous?.size ?? 0,
    transferred: payload.transferred ?? previous?.transferred ?? 0,
    state: payload.state ?? fallbackState,
    direction: payload.direction ?? previous?.direction ?? 'received',
    path: payload.path ?? previous?.path,
    files: payload.files ?? previous?.files,
    directories: payload.directories ?? previous?.directories,
  };
  return [...old.filter((item) => item.id !== payload.id), next];
}
