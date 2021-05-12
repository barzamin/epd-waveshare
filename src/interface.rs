use crate::traits::Command;
use crate::Error;
use core::marker::PhantomData;
use embedded_hal::{
    blocking::{delay::*, spi::Write},
    digital::*,
};

/// The Connection Interface of all (?) Waveshare EPD-Devices
///
pub(crate) struct DisplayInterface<SPI, CS, BUSY, DC, RST, DELAY> {
    /// SPI
    _spi: PhantomData<SPI>,
    /// DELAY
    _delay: PhantomData<DELAY>,
    /// CS for SPI
    cs: CS,
    /// Low for busy, Wait until display is ready!
    busy: BUSY,
    /// Data/Command Control Pin (High for data, Low for command)
    dc: DC,
    /// Pin for Resetting
    rst: RST,
}

impl<S, P, SPI, CS, BUSY, DC, RST, DELAY> DisplayInterface<SPI, CS, BUSY, DC, RST, DELAY>
where
    SPI: Write<u8, Error=S>,
    CS: OutputPin<Error=P>,
    BUSY: InputPin<Error=P>,
    DC: OutputPin<Error=P>,
    RST: OutputPin<Error=P>,
    DELAY: DelayMs<u8>,
{
    pub fn new(cs: CS, busy: BUSY, dc: DC, rst: RST) -> Self {
        DisplayInterface {
            _spi: PhantomData::default(),
            _delay: PhantomData::default(),
            cs,
            busy,
            dc,
            rst,
        }
    }

    /// Basic function for sending [Commands](Command).
    ///
    /// Enables direct interaction with the device with the help of [data()](DisplayInterface::data())
    pub(crate) fn cmd<T: Command>(&mut self, spi: &mut SPI, command: T) -> Result<(), Error<S, P, DELAY::Error>> {
        // low for commands
        let _ = self.dc.try_set_low().map_err(Error::PinError)?;

        // Transfer the command over spi
        self.write(spi, &[command.address()])?;

        Ok(())
    }

    /// Basic function for sending an array of u8-values of data over spi
    ///
    /// Enables direct interaction with the device with the help of [command()](Epd4in2::command())
    pub(crate) fn data(&mut self, spi: &mut SPI, data: &[u8]) -> Result<(), Error<S, P, DELAY::Error>> {
        // high for data
        let _ = self.dc.try_set_high().map_err(Error::PinError)?;

        // Transfer data (u8-array) over spi
        self.write(spi, data)
    }

    /// Basic function for sending [Commands](Command) and the data belonging to it.
    ///
    /// TODO: directly use ::write? cs wouldn't needed to be changed twice than
    pub(crate) fn cmd_with_data<T: Command>(
        &mut self,
        spi: &mut SPI,
        command: T,
        data: &[u8],
    ) -> Result<(), Error<S, P, DELAY::Error>> {
        self.cmd(spi, command)?;
        self.data(spi, data)
    }

    /// Basic function for sending the same byte of data (one u8) multiple times over spi
    ///
    /// Enables direct interaction with the device with the help of [command()](ConnectionInterface::command())
    pub(crate) fn data_x_times(
        &mut self,
        spi: &mut SPI,
        val: u8,
        repetitions: u32,
    ) -> Result<(), Error<S, P, DELAY::Error>> {
        // high for data
        let _ = self.dc.try_set_high().map_err(Error::PinError)?;
        // Transfer data (u8) over spi
        for _ in 0..repetitions {
            self.write(spi, &[val])?;
        }
        Ok(())
    }

    // spi write helper/abstraction function
    fn write(&mut self, spi: &mut SPI, data: &[u8]) -> Result<(), Error<S, P, DELAY::Error>> {
        // activate spi with cs low
        let _ = self.cs.try_set_low().map_err(Error::PinError)?;

        // transfer spi data
        // Be careful!! Linux has a default limit of 4096 bytes per spi transfer
        // see https://raspberrypi.stackexchange.com/questions/65595/spi-transfer-fails-with-buffer-size-greater-than-4096
        if cfg!(target_os = "linux") {
            for data_chunk in data.chunks(4096) {
                spi.try_write(data_chunk).map_err(Error::SPIError)?;
            }
        } else {
            spi.try_write(data).map_err(Error::SPIError)?;
        }

        // deactivate spi with cs high
        let _ = self.cs.try_set_high().map_err(Error::PinError)?;

        Ok(())
    }

    /// Waits until device isn't busy anymore (busy == HIGH)
    ///
    /// This is normally handled by the more complicated commands themselves,
    /// but in the case you send data and commands directly you might need to check
    /// if the device is still busy
    ///
    /// is_busy_low
    ///
    ///  - TRUE for epd4in2, epd2in13, epd2in7, epd5in83, epd7in5
    ///  - FALSE for epd2in9, epd1in54 (for all Display Type A ones?)
    ///
    /// Most likely there was a mistake with the 2in9 busy connection
    /// //TODO: use the #cfg feature to make this compile the right way for the certain types
    pub(crate) fn wait_until_idle(&mut self, is_busy_low: bool) -> Result<(), Error<S, P, DELAY::Error>> {
        // //tested: worked without the delay for all tested devices
        // //self.try_delay_ms(1);
        while self.is_busy(is_busy_low)? {
            // //tested: REMOVAL of DELAY: it's only waiting for the signal anyway and should continue work asap
            // //old: shorten the time? it was 100 in the beginning
            // //self.try_delay_ms(5);
        }

        Ok(())
    }

    /// Checks if device is still busy
    ///
    /// This is normally handled by the more complicated commands themselves,
    /// but in the case you send data and commands directly you might need to check
    /// if the device is still busy
    ///
    /// is_busy_low
    ///
    ///  - TRUE for epd4in2, epd2in13, epd2in7, epd5in83, epd7in5
    ///  - FALSE for epd2in9, epd1in54 (for all Display Type A ones?)
    ///
    /// Most likely there was a mistake with the 2in9 busy connection
    /// //TODO: use the #cfg feature to make this compile the right way for the certain types
    pub(crate) fn is_busy(&self, is_busy_low: bool) -> Result<bool, Error<S, P, DELAY::Error>> {
        Ok((is_busy_low && self.busy.try_is_low().map_err(Error::PinError)?)
            || (!is_busy_low && self.busy.try_is_high().map_err(Error::PinError)?))
    }

    /// Resets the device.
    ///
    /// Often used to awake the module from deep sleep. See [Epd4in2::sleep()](Epd4in2::sleep())
    ///
    /// The timing of keeping the reset pin low seems to be important and different per device.
    /// Most displays seem to require keeping it low for 10ms, but the 7in5_v2 only seems to reset
    /// properly with 2ms
    pub(crate) fn reset(&mut self, delay: &mut DELAY, duration: u8) -> Result<(), Error<S, P, DELAY::Error>> {
        let _ = self.rst.try_set_high().map_err(Error::PinError)?;
        delay.try_delay_ms(200).map_err(Error::DelayError)?;

        let _ = self.rst.try_set_low().map_err(Error::PinError)?;
        delay.try_delay_ms(duration).map_err(Error::DelayError)?;
        let _ = self.rst.try_set_high().map_err(Error::PinError)?;
        //TODO: the upstream libraries always sleep for 200ms here
        // 10ms works fine with just for the 7in5_v2 but this needs to be validated for other devices
        delay.try_delay_ms(200).map_err(Error::DelayError)?;

        Ok(())
    }
}
