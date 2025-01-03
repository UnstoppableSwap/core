import { Box } from "@material-ui/core";
import { SwapState } from "models/storeModel";
import CircularProgressWithSubtitle from "../CircularProgressWithSubtitle";
import BitcoinPunishedPage from "./done/BitcoinPunishedPage";
import BitcoinRefundedPage from "./done/BitcoinRefundedPage";
import XmrRedeemInMempoolPage from "./done/XmrRedeemInMempoolPage";
import ProcessExitedPage from "./exited/ProcessExitedPage";
import BitcoinCancelledPage from "./in_progress/BitcoinCancelledPage";
import BitcoinLockTxInMempoolPage from "./in_progress/BitcoinLockTxInMempoolPage";
import BitcoinRedeemedPage from "./in_progress/BitcoinRedeemedPage";
import CancelTimelockExpiredPage from "./in_progress/CancelTimelockExpiredPage";
import EncryptedSignatureSentPage from "./in_progress/EncryptedSignatureSentPage";
import ReceivedQuotePage from "./in_progress/ReceivedQuotePage";
import SwapSetupInflightPage from "./in_progress/SwapSetupInflightPage";
import XmrLockedPage from "./in_progress/XmrLockedPage";
import XmrLockTxInMempoolPage from "./in_progress/XmrLockInMempoolPage";
import InitPage from "./init/InitPage";
import WaitingForBitcoinDepositPage from "./init/WaitingForBitcoinDepositPage";

export default function SwapStatePage({ state }: { state: SwapState | null }) {
  if (state === null) {
    return <InitPage />;
  }

  switch (state.curr.type) {
    case "RequestingQuote":
      return <CircularProgressWithSubtitle description="Requesting quote..." />;
    case "Resuming":
      return <CircularProgressWithSubtitle description="Resuming swap..." />;
    case "ReceivedQuote":
      return <ReceivedQuotePage />;
    case "WaitingForBtcDeposit":
      return <WaitingForBitcoinDepositPage {...state.curr.content} />;
    case "SwapSetupInflight":
      return <SwapSetupInflightPage {...state.curr.content} />;
    case "BtcLockTxInMempool":
      return <BitcoinLockTxInMempoolPage {...state.curr.content} />;
    case "XmrLockTxInMempool":
      return <XmrLockTxInMempoolPage {...state.curr.content} />;
    case "XmrLocked":
      return <XmrLockedPage />;
    case "EncryptedSignatureSent":
      return <EncryptedSignatureSentPage />;
    case "BtcRedeemed":
      return <BitcoinRedeemedPage />;
    case "XmrRedeemInMempool":
      return <XmrRedeemInMempoolPage {...state.curr.content} />;
    case "CancelTimelockExpired":
      return <CancelTimelockExpiredPage />;
    case "BtcCancelled":
      return <BitcoinCancelledPage />;
    case "BtcRefunded":
      return <BitcoinRefundedPage {...state.curr.content} />;
    case "BtcPunished":
      return <BitcoinPunishedPage state={state.curr} />;
    case "AttemptingCooperativeRedeem":
      return (
        <CircularProgressWithSubtitle description="Attempting to redeem the Monero with the help of the other party" />
      );
    case "CooperativeRedeemAccepted":
      return (
        <CircularProgressWithSubtitle description="The other party is cooperating with us to redeem the Monero..." />
      );
    case "CooperativeRedeemRejected":
      return <BitcoinPunishedPage state={state.curr} />;
    case "Released":
      return <ProcessExitedPage prevState={state.prev} swapId={state.swapId} />;
    default:
      // TODO: Use this when we have all states implemented, ensures we don't forget to implement a state
      // return exhaustiveGuard(state.curr.type);
      return (
        <Box>
          No information to display
          <br />
          State: {JSON.stringify(state, null, 4)}
        </Box>
      );
  }
}
