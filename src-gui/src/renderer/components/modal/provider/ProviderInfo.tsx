import { Box, Chip, Divider, makeStyles, Tooltip, Typography } from "@material-ui/core";
import { VerifiedUser } from "@material-ui/icons";
import { ExtendedProviderStatus } from "models/apiModel";
import TruncatedText from "renderer/components/other/TruncatedText";
import {
  MoneroBitcoinExchangeRate,
  SatsAmount,
} from "renderer/components/other/Units";
import { satsToBtc, secondsToDays } from "utils/conversionUtils";
import { isProviderOutdated } from 'utils/multiAddrUtils';
import WarningIcon from '@material-ui/icons/Warning';
import { useAppSelector } from "store/hooks";
import { BobStateName } from "models/tauriModelExt";

const useStyles = makeStyles((theme) => ({
  content: {
    flex: 1,
    "& *": {
      lineBreak: "anywhere",
    },
  },
  chipsOuter: {
    display: "flex",
    marginTop: theme.spacing(1),
    gap: theme.spacing(0.5),
    flexWrap: "wrap",
  },
}));

/**
 * A chip that displays the markup of the provider's exchange rate compared to the market rate.
 */
function ProviderMarkupChip({ provider }: { provider: ExtendedProviderStatus }) {
  const marketExchangeRate = useAppSelector(s => s.rates?.xmrBtcRate);
  if (marketExchangeRate === null)
    return null;

  const providerExchangeRate = satsToBtc(provider.price);
  /** The markup of the exchange rate compared to the market rate in percent */
  const markup = (providerExchangeRate - marketExchangeRate) / marketExchangeRate * 100;

  return (
    <Tooltip title="The markup this provider charges compared to centralized markets. A lower markup means that you get more Monero for your Bitcoin.">
      <Chip label={`Markup ${markup.toFixed(2)}%`} />
    </Tooltip>
  );

}

/**
 * A component that displays our local reputation of the provider.
 */
function ProviderReputation({ provider }: { provider: ExtendedProviderStatus }) {
  const swapsByProvider = useAppSelector(s => Object.values(s.rpc.state.swapInfos).filter(s => s.seller.peer_id === provider.peerId));

  const successfullSwaps = swapsByProvider.filter(s => s.state_name === BobStateName.XmrRedeemed).length;
  const refundedSwaps = swapsByProvider.filter(s => s.state_name === BobStateName.BtcRefunded).length;
  const failedSwaps = swapsByProvider.filter(s => s.state_name === BobStateName.BtcPunished).length;


  if (successfullSwaps + refundedSwaps + failedSwaps === 0) {
    return null;
  }

  return <Tooltip title="How many swaps you've made with this provider, how many were successful, and how many were refunded">
    <Chip label={
      <Box display="flex" style={{ gap: "0.5rem" }}>
        <Box color="success.main">{successfullSwaps} success</Box>
        <Divider orientation="vertical" flexItem />
        <Box color="warning.main">{refundedSwaps} refund</Box>
        <Divider orientation="vertical" flexItem />
        <Box color="error.main">{failedSwaps} fail</Box>
      </Box>
    } />
  </Tooltip>
}

export default function ProviderInfo({
  provider,
}: {
  provider: ExtendedProviderStatus;
}) {
  const classes = useStyles();
  const isOutdated = isProviderOutdated(provider);

  return (
    <Box className={classes.content}>
      <Typography color="textSecondary" gutterBottom>
        Swap Provider
      </Typography>
      <Typography variant="h5" component="h2">
        {provider.multiAddr}
      </Typography>
      <Typography color="textSecondary" gutterBottom>
        <TruncatedText limit={16} truncateMiddle>{provider.peerId}</TruncatedText>
      </Typography>
      <Typography variant="caption">
        Exchange rate:{" "}
        <MoneroBitcoinExchangeRate rate={satsToBtc(provider.price)} />
        <br />
        Minimum swap amount: <SatsAmount amount={provider.minSwapAmount} />
        <br />
        Maximum swap amount: <SatsAmount amount={provider.maxSwapAmount} />
      </Typography>
      <Box className={classes.chipsOuter}>
        <ProviderReputation provider={provider} />
        {provider.testnet && <Chip label="Testnet" />}
        {provider.uptime && (
          <Tooltip title="A high uptime (>90%) indicates reliability. Providers with very low uptime may be unreliable and cause swaps to take longer to complete or fail entirely.">
            <Chip label={`${Math.round(provider.uptime * 100)}% uptime`} />
          </Tooltip>
        )}
        {provider.age ? (
          <Chip
            label={`Went online ${Math.round(secondsToDays(provider.age))} ${provider.age === 1 ? "day" : "days"
              } ago`}
          />
        ) : (
          <Chip label="Discovered via rendezvous point" />
        )}
        {provider.recommended === true && (
          <Tooltip title="This provider has shown to be exceptionally reliable">
            <Chip label="Recommended" icon={<VerifiedUser />} color="primary" />
          </Tooltip>
        )}
        {isOutdated && (
          <Tooltip title="This provider is running an older version of the software. Outdated providers may be unreliable and cause swaps to take longer to complete or fail entirely.">
            <Chip label="Outdated" icon={<WarningIcon />} color="primary" />
          </Tooltip>
        )}
        <ProviderMarkupChip provider={provider} />
      </Box>
    </Box>
  );
}
