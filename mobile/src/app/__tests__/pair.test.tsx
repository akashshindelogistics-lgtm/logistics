import { render, screen, userEvent } from '@testing-library/react-native';
import { Alert } from 'react-native';
import Pair from '../pair';

jest.mock('expo-router', () => ({
  router: { replace: jest.fn() },
}));
jest.mock('../../lib/storage', () => ({
  savePairing: jest.fn().mockResolvedValue(undefined),
}));

import { router } from 'expo-router';
import { savePairing } from '../../lib/storage';

const mockReplace = router.replace as jest.Mock;
const mockSavePairing = savePairing as jest.Mock;

beforeEach(() => {
  jest.clearAllMocks();
  jest.spyOn(Alert, 'alert').mockImplementation(() => {});
});

describe('<Pair />', () => {
  it('saves a valid token and server address, then moves to the status screen', async () => {
    const user = userEvent.setup();
    await render(<Pair />);

    await user.type(screen.getByTestId('device-token-input'), '11111111-2222-3333-4444-555555555555');
    await user.type(screen.getByTestId('api-base-url-input'), 'https://api.example.com/');
    await user.press(screen.getByTestId('save-pairing-button'));

    expect(mockSavePairing).toHaveBeenCalledWith({
      deviceToken: '11111111-2222-3333-4444-555555555555',
      apiBaseUrl: 'https://api.example.com', // trailing slash stripped
    });
    expect(mockReplace).toHaveBeenCalledWith('/status');
  });

  it('rejects a token that is not a UUID without saving anything', async () => {
    const user = userEvent.setup();
    await render(<Pair />);

    await user.type(screen.getByTestId('device-token-input'), 'not-a-token');
    await user.type(screen.getByTestId('api-base-url-input'), 'https://api.example.com');
    await user.press(screen.getByTestId('save-pairing-button'));

    expect(Alert.alert).toHaveBeenCalled();
    expect(mockSavePairing).not.toHaveBeenCalled();
    expect(mockReplace).not.toHaveBeenCalled();
  });

  it('rejects a server address with no scheme', async () => {
    const user = userEvent.setup();
    await render(<Pair />);

    await user.type(screen.getByTestId('device-token-input'), '11111111-2222-3333-4444-555555555555');
    await user.type(screen.getByTestId('api-base-url-input'), 'api.example.com');
    await user.press(screen.getByTestId('save-pairing-button'));

    expect(Alert.alert).toHaveBeenCalled();
    expect(mockSavePairing).not.toHaveBeenCalled();
  });
});
