import { Box, makeStyles, Typography } from "@material-ui/core";
import InfoBox from "renderer/components/modal/swap/InfoBox";
import { useSettings } from "store/hooks";
import { Search } from "@material-ui/icons";
import PromiseInvokeButton from "renderer/components/PromiseInvokeButton";
import { listSellersAtRendezvousPoint } from "renderer/rpc";
import { useAppDispatch } from "store/hooks";
import { discoveredMakersByRendezvous } from "store/features/makersSlice";
import { useSnackbar } from "notistack";

const useStyles = makeStyles((theme) => ({
  title: {
    display: "flex",
    alignItems: "center",
    gap: theme.spacing(1),
  },
  button: {
    marginTop: theme.spacing(2),
  }
}));

export default function DiscoveryBox() {
  const classes = useStyles();
  const rendezvousPoints = useSettings((s) => s.rendezvousPoints);
  const dispatch = useAppDispatch();
  const { enqueueSnackbar } = useSnackbar();

  const handleDiscovery = async () => {
    const response = await listSellersAtRendezvousPoint(rendezvousPoints);
    dispatch(discoveredMakersByRendezvous(response.sellers));

    enqueueSnackbar(`Discovered ${response.sellers.length} makers. ${response.sellers.filter((seller) => seller.status.type === "Online").length} of which are online, ${response.sellers.filter((seller) => seller.status.type === "Unreachable").length} of which are unreachable.`, { variant: "success" });

    return response.sellers.length;
  };

  return (
    <InfoBox
      title={
        <Box className={classes.title}>
          Discover Makers
        </Box>
      }
      mainContent={
        <Typography variant="subtitle2">
          By connecting to rendezvous points run by volunteers, you can discover makers and then connect and swap with them in a decentralized manner.
          
          You have {rendezvousPoints.length} stored rendezvous {rendezvousPoints.length === 1 ? 'point' : 'points'} which we will connect to. We will also attempt to connect to peers which you have previously connected to.
        </Typography>
      }
      additionalContent={
        <PromiseInvokeButton
          variant="contained"
          color="primary"
          onInvoke={handleDiscovery}
          disabled={rendezvousPoints.length === 0}
          startIcon={<Search />}
          className={classes.button}
          displayErrorSnackbar
        >
          Discover Makers
        </PromiseInvokeButton>
      }
      icon={null}
      loading={false}
    />
  );
} 