import { Box, Button, LinearProgress, makeStyles } from "@material-ui/core";
import { Alert } from "@material-ui/lab";
import { useNavigate } from "react-router-dom";
import { useAppSelector, usePendingBackgroundProcesses } from "store/hooks";
import { exhaustiveGuard } from "utils/typescriptUtils";
import { LoadingSpinnerAlert } from "./LoadingSpinnerAlert";
import { bytesToMb } from "utils/conversionUtils";
import { TauriBackgroundProgress, TauriContextStatusEvent } from "models/tauriModel";
import { useEffect, useRef, useState } from "react";
import TruncatedText from "../other/TruncatedText";

const useStyles = makeStyles((theme) => ({
  innerAlert: {
    display: "flex",
    flexDirection: "column",
    gap: theme.spacing(2),
  },
}));

function AlertWithLinearProgress({ title, progress }: {
  title: string,
  progress: number | null,
}) {
  const BUFFER_PROGRESS_ADDITION_MAX = 20;

  const [bufferProgressAddition, setBufferProgressAddition] = useState(Math.random() * BUFFER_PROGRESS_ADDITION_MAX);

  useEffect(() => {
    setBufferProgressAddition(Math.random() * BUFFER_PROGRESS_ADDITION_MAX);
  }, [progress]);

  // If the progress is already at 100%, but not finished yet we show an indeterminate progress bar
  // as it'd be confusing to show a 100% progress bar for longer than a second or so.
  return <Alert severity="info">
    <Box style={{ display: "flex", flexDirection: "column", gap: "0.5rem" }}>
      {title}
      {(progress === null || progress === 0 || progress >= 100) ? (
        <LinearProgress variant="indeterminate" />
      ) : (
        <LinearProgress variant="buffer" value={progress} valueBuffer={Math.min(progress + bufferProgressAddition, 100)} />
      )}
    </Box>
  </Alert>
}

function PartialInitStatus({ status, currentNumOfType, totalOfType, classes }: {
  status: TauriBackgroundProgress,
  currentNumOfType: number,
  totalOfType: number,
  classes: ReturnType<typeof useStyles>
}) {
  if (status.progress.type === "Completed") {
    return null;
  }

  const titleSuffix = totalOfType > 1 ? ` (${currentNumOfType}/${totalOfType})` : "";

  switch (status.componentName) {
    case "EstablishingTorCircuits":
      console.log("EstablishingTorCircuits", status.progress.content);
      return <AlertWithLinearProgress title={`Establishing Tor circuits${titleSuffix}`} progress={status.progress.content.frac * 100} />
    case "SyncingBitcoinWallet":
      const progressValue =
        status.progress.content?.type === "Known" ? 
        (status.progress.content?.content?.consumed / status.progress.content?.content?.total) * 100 : null;

      return <AlertWithLinearProgress title={`Syncing Bitcoin wallet${titleSuffix}`} progress={progressValue} />
    case "FullScanningBitcoinWallet":
      const fullScanProgressValue = status.progress.content?.type === "Known" ? (status.progress.content?.content?.current_index / 1250) * 100 : null;
      return <AlertWithLinearProgress title={`Full scan of Bitcoin wallet (one time operation)`} progress={fullScanProgressValue} />
    case "OpeningBitcoinWallet":
      return (
        <LoadingSpinnerAlert severity="info">
          {`Opening Bitcoin wallet${titleSuffix}`}
        </LoadingSpinnerAlert>
      );
    case "DownloadingMoneroWalletRpc":
      return <AlertWithLinearProgress title={`Downloading and verifying the Monero wallet RPC (${bytesToMb(status.progress.content.size).toFixed(2)} MB)${titleSuffix}`} progress={status.progress.content.progress} />
    case "OpeningMoneroWallet":
      return (
        <LoadingSpinnerAlert severity="info">
          {`Opening the Monero wallet${titleSuffix}`}
        </LoadingSpinnerAlert>
      );
    case "OpeningDatabase":
      return (
        <LoadingSpinnerAlert severity="info">
          {`Opening the local database${titleSuffix}`}
        </LoadingSpinnerAlert>
      );
    case "BackgroundRefund":
      return <LoadingSpinnerAlert severity="info">
        {`Refunding swap `}<TruncatedText limit={10}>{status.progress.content.swap_id}</TruncatedText>
      </LoadingSpinnerAlert>
    default:
      return exhaustiveGuard(status);
  }
}

export default function DaemonStatusAlert() {
  const contextStatus = useAppSelector((s) => s.rpc.status);
  const navigate = useNavigate();

  if (contextStatus === null || contextStatus === TauriContextStatusEvent.NotInitialized) {
    return <LoadingSpinnerAlert severity="warning">Checking for available remote nodes</LoadingSpinnerAlert>;
  }

  switch (contextStatus) {
    case TauriContextStatusEvent.Initializing:
      return <LoadingSpinnerAlert severity="warning">Core components are loading</LoadingSpinnerAlert>;
    case TauriContextStatusEvent.Available:
      return <Alert severity="success">The daemon is running</Alert>;
    case TauriContextStatusEvent.Failed:
      return (
        <Alert
          severity="error"
          action={
            <Button
              size="small"
              variant="outlined"
              onClick={() => navigate("/settings#daemon-control-box")}
            >
              View Logs
            </Button>
          }
        >
          The daemon has stopped unexpectedly
        </Alert>
      );
    default:
      return exhaustiveGuard(contextStatus);
  }
}

export function BackgroundProgressAlerts() {
  const backgroundProgress = usePendingBackgroundProcesses();
  const classes = useStyles();

  if (backgroundProgress.length === 0) {
    return null;
  }

  const componentCounts: Record<string, number> = {};
  backgroundProgress.forEach(([, status]) => {
    componentCounts[status.componentName] = (componentCounts[status.componentName] || 0) + 1;
  });

  const renderedComponentNames = new Set<string>();
  const uniqueBackgroundProcesses = backgroundProgress.filter(([, status]) => {
    if (!renderedComponentNames.has(status.componentName)) {
      renderedComponentNames.add(status.componentName);
      return true;
    }
    return false;
  });

  return uniqueBackgroundProcesses.map(([id, status]) => (
    <PartialInitStatus
      key={id}
      status={status}
      classes={classes}
      currentNumOfType={1} // Since we only show the first one
      totalOfType={componentCounts[status.componentName]}
    />
  ));
}