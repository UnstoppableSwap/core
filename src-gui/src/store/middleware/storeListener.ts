import { createListenerMiddleware } from "@reduxjs/toolkit";
import { getAllSwapInfos, checkBitcoinBalance, updateAllNodeStatuses, fetchSellersAtPresetRendezvousPoints } from "renderer/rpc";
import logger from "utils/logger";
import { contextStatusEventReceived } from "store/features/rpcSlice";
import { addNode, setFetchFiatPrices, setFiatCurrency } from "store/features/settingsSlice";
import { updateRates } from "renderer/api";
import { store } from "renderer/store/storeRenderer";

export function createMainListeners() {
  const listener = createListenerMiddleware();

  // Listener for when the Context becomes available
  // When the context becomes available, we check the bitcoin balance, fetch all swap infos and connect to the rendezvous point
  listener.startListening({
    actionCreator: contextStatusEventReceived,
    effect: async (action) => {
      const status = action.payload;

      // If the context is available, check the bitcoin balance and fetch all swap infos
      if (status.type === "Available") {
        logger.debug(
          "Context is available, checking bitcoin balance and history",
        );
        await Promise.allSettled([
          checkBitcoinBalance(),
          getAllSwapInfos(),
          fetchSellersAtPresetRendezvousPoints(),
        ]);
      }
    },
  });

  // Update the rates when the fiat currency is changed
  listener.startListening({
    actionCreator: setFiatCurrency,
    effect: async () => {
      if (store.getState().settings.fetchFiatPrices) {
        logger.info("Fiat currency changed, updating rates...");
        await updateRates();
      }
    },
  });

  // Update the rates when fetching fiat prices is enabled
  listener.startListening({
    actionCreator: setFetchFiatPrices,
    effect: async (action) => {
      if (action.payload === true) {
        logger.info("Activated fetching fiat prices, updating rates...");
        await updateRates();
      }
    },
  });

  // Update the node status when a new one is added
  listener.startListening({
    actionCreator: addNode,
    effect: async (_) => {
      await updateAllNodeStatuses();
    },
  });

  return listener;
}
