import RefreshIcon from "@mui/icons-material/Refresh";
import PromiseInvokeButton from "renderer/components/PromiseInvokeButton";
import { checkBitcoinBalance } from "renderer/rpc";

export default function WalletRefreshButton() {
  return (
    <PromiseInvokeButton
      endIcon={<RefreshIcon />}
      isIconButton
      onInvoke={() => checkBitcoinBalance()}
      size="small"
    />
  );
}
