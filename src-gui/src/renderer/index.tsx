import ReactDOM from 'react-dom';
import { Provider } from "react-redux";
import { PersistGate } from "redux-persist/integration/react";
import App from "./components/App";
import { persistor, store } from "./store/storeRenderer";

ReactDOM.render(
  <Provider store={store}>
    <PersistGate loading={null} persistor={persistor}>
      <App />
    </PersistGate>
  </Provider>,
  document.getElementById('root')
);
