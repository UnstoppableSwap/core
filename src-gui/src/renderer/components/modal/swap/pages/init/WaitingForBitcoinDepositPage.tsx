import { Box, Typography, Paper, Divider, LinearProgress } from "@mui/material";
import { TauriSwapProgressEventContent } from "models/tauriModelExt";
import BitcoinIcon from "../../../../icons/BitcoinIcon";
import { SatsAmount, MoneroBitcoinExchangeRate } from "../../../../other/Units";
import DepositAddressInfoBox from "../../DepositAddressInfoBox";
import { Alert } from "@mui/material";
import {
  usePendingSelectMakerApproval,
  usePendingBackgroundProcesses,
} from "store/hooks";
import PromiseInvokeButton from "renderer/components/PromiseInvokeButton";
import { resolveApproval } from "renderer/rpc";
import IdentIcon from "renderer/components/icons/IdentIcon";
import TruncatedText from "renderer/components/other/TruncatedText";
import { satsToBtc } from "utils/conversionUtils";

function MakerDiscoveryStatus() {
  const backgroundProcesses = usePendingBackgroundProcesses();

  // Find active ListSellers processes
  const listSellersProcesses = backgroundProcesses.filter(
    ([, status]) =>
      status.componentName === "ListSellers" &&
      status.progress.type === "Pending",
  );

  const isActive = listSellersProcesses.length > 0;

  // Default values for inactive state
  let progress = {
    rendezvous_points_total: 0,
    peers_discovered: 0,
    rendezvous_points_connected: 0,
    quotes_received: 0,
    quotes_failed: 0,
  };
  let progressValue = 0;

  if (isActive) {
    // Use the first ListSellers process for display
    const [, status] = listSellersProcesses[0];

    // Type guard to ensure we have ListSellers progress
    if (
      status.componentName === "ListSellers" &&
      status.progress.type === "Pending"
    ) {
      progress = status.progress.content;

      const totalExpected =
        progress.rendezvous_points_total + progress.peers_discovered;
      const totalCompleted =
        progress.rendezvous_points_connected +
        progress.quotes_received +
        progress.quotes_failed;
      progressValue =
        totalExpected > 0 ? (totalCompleted / totalExpected) * 100 : 0;
    }
  }

  return (
    <Box
      sx={{
        width: "100%",
        mb: 2,
        p: 2,
        border: "1px solid",
        borderColor: isActive ? "info.main" : "divider",
        borderRadius: 1,
        opacity: isActive ? 1 : 0.6,
      }}
    >
      <Box
        sx={{
          display: "flex",
          flexDirection: "column",
          gap: 1.5,
          width: "100%",
        }}
      >
        <Box
          sx={{
            display: "flex",
            alignItems: "center",
            justifyContent: "space-between",
            width: "100%",
          }}
        >
          <Typography
            variant="body2"
            sx={{
              fontWeight: "medium",
              color: isActive ? "info.main" : "text.disabled",
            }}
          >
            {isActive
              ? "Getting offers..."
              : "Waiting a few seconds before refreshing offers"}
          </Typography>
          <Box sx={{ display: "flex", gap: 2 }}>
            <Typography
              variant="caption"
              sx={{
                color: isActive ? "success.main" : "text.disabled",
                fontWeight: "medium",
              }}
            >
              {progress.quotes_received} online
            </Typography>
            <Typography
              variant="caption"
              sx={{
                color: isActive ? "error.main" : "text.disabled",
                fontWeight: "medium",
              }}
            >
              {progress.quotes_failed} offline
            </Typography>
          </Box>
        </Box>
        <LinearProgress
          variant="determinate"
          value={Math.min(progressValue, 100)}
          sx={{
            width: "100%",
            height: 8,
            borderRadius: 4,
            opacity: isActive ? 1 : 0.4,
            backgroundColor: isActive ? "info.light" : "action.disabled",
          }}
        />
      </Box>
    </Box>
  );
}

export default function WaitingForBtcDepositPage({
  deposit_address,
  min_bitcoin_lock_tx_fee,
  max_giveable,
}: TauriSwapProgressEventContent<"WaitingForBtcDeposit">) {
  const pendingSelectMakerApprovals = usePendingSelectMakerApproval();

  return (
    <Box sx={{ display: "flex", flexDirection: "column", gap: 1 }}>
      <MakerDiscoveryStatus />
      <Box sx={{ display: "flex", flexDirection: "rows", gap: 1 }}>
        <DepositAddressInfoBox
          title="Bitcoin Deposit Address"
          address={deposit_address}
          icon={<BitcoinIcon />}
          additionalContent={null}
        />

        {/* Balance and Fee Section */}
        <Paper variant="outlined" sx={{ p: 2 }}>
          <Box sx={{ display: "flex", flexDirection: "column", gap: 1 }}>
            <Typography variant="body1">
              <strong>Available Balance:</strong>{" "}
              <SatsAmount amount={max_giveable} />
            </Typography>
            <Typography variant="body1">
              <strong>Network Fee:</strong> ≈{" "}
              <SatsAmount amount={min_bitcoin_lock_tx_fee} />
            </Typography>
          </Box>
        </Paper>
      </Box>

      {/* Offers Section */}
      <Box>
        {pendingSelectMakerApprovals.length > 0 && (
          <>
            <Box sx={{ display: "flex", flexDirection: "column", gap: 1 }}>
              {pendingSelectMakerApprovals.map((makerApproval, index) => {
                const { request_id, details } = makerApproval.content;
                const { maker } = details.content;

                return (
                  <Box key={request_id}>
                    <Box
                      sx={{
                        display: "flex",
                        alignItems: "center",
                        gap: 2,
                        py: 1.5,
                        px: 2,
                        backgroundColor: "background.default",
                        borderRadius: 1,
                        border: "1px solid",
                        borderColor: "divider",
                      }}
                    >
                      {/* Icon and ID */}
                      <Box
                        sx={{
                          display: "flex",
                          alignItems: "center",
                          gap: 1.5,
                          flex: 1,
                        }}
                      >
                        <IdentIcon value={maker.peer_id} size="2rem" />
                        <Box sx={{ minWidth: 0 }}>
                          <Typography
                            variant="body2"
                            sx={{ fontWeight: "medium" }}
                          >
                            <TruncatedText limit={12} truncateMiddle>
                              {maker.peer_id}
                            </TruncatedText>
                          </Typography>
                          <Typography variant="caption" color="textSecondary">
                            v{maker.version}
                          </Typography>
                        </Box>
                      </Box>

                      {/* Price */}
                      <Box sx={{ textAlign: "center", minWidth: 120 }}>
                        <Typography
                          variant="body2"
                          sx={{ fontWeight: "medium" }}
                        >
                          <MoneroBitcoinExchangeRate
                            rate={satsToBtc(maker.quote.price)}
                          />
                        </Typography>
                        <Typography variant="caption" color="textSecondary">
                          <SatsAmount amount={maker.quote.min_quantity} /> -{" "}
                          <SatsAmount amount={maker.quote.max_quantity} />
                        </Typography>
                      </Box>

                      {/* Accept Button */}
                      <PromiseInvokeButton
                        variant="contained"
                        color="success"
                        size="large"
                        sx={{ minWidth: 100, fontWeight: "bold" }}
                        onInvoke={() => resolveApproval(request_id, true)}
                        displayErrorSnackbar
                      >
                        Accept
                      </PromiseInvokeButton>
                    </Box>

                    {index < pendingSelectMakerApprovals.length - 1 && (
                      <Divider sx={{ my: 1 }} />
                    )}
                  </Box>
                );
              })}
            </Box>
          </>
        )}

        {pendingSelectMakerApprovals.length === 0 && (
          <Paper variant="outlined" sx={{ p: 3, textAlign: "center" }}>
            <Typography variant="body1" color="textSecondary">
              Searching for available makers...
            </Typography>
            <Typography variant="body2" color="textSecondary" sx={{ mt: 1 }}>
              Please wait while we find the best offers for your swap.
            </Typography>
          </Paper>
        )}
      </Box>
    </Box>
  );
}
