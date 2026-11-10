package br.org.oficinadasmeninas.benchhidden;

import br.org.oficinadasmeninas.domain.donation.dto.DonationDto;
import br.org.oficinadasmeninas.domain.donation.service.IDonationService;
import br.org.oficinadasmeninas.domain.payment.PaymentMethodEnum;
import br.org.oficinadasmeninas.domain.payment.PaymentStatusEnum;
import br.org.oficinadasmeninas.domain.payment.dto.PaymentDto;
import br.org.oficinadasmeninas.domain.payment.dto.PaymentNotificationDto;
import br.org.oficinadasmeninas.domain.payment.service.IPaymentService;
import br.org.oficinadasmeninas.domain.paymentgateway.PaymentGatewayEnum;
import br.org.oficinadasmeninas.domain.paymentgateway.dto.PaymentChargesDto;
import br.org.oficinadasmeninas.domain.sponsorship.dto.SponsorshipDto;
import br.org.oficinadasmeninas.domain.sponsorship.service.ISponsorshipService;
import br.org.oficinadasmeninas.domain.user.dto.UserDto;
import br.org.oficinadasmeninas.infra.logspagbank.service.LogPagbankService;
import br.org.oficinadasmeninas.infra.paymentgateway.pagbank.dto.ResponseWebhookCard;
import br.org.oficinadasmeninas.infra.paymentgateway.pagbank.dto.ResponseWebhookCustomer;
import br.org.oficinadasmeninas.infra.paymentgateway.pagbank.dto.ResponseWebhookPaymentMethod;
import br.org.oficinadasmeninas.infra.paymentgateway.pagbank.mappers.RequestCreateCheckoutPagbankMapper;
import br.org.oficinadasmeninas.infra.paymentgateway.pagbank.service.PaymentGatewayService;
import br.org.oficinadasmeninas.infra.user.service.UserService;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.mockito.Mockito;
import org.springframework.http.HttpHeaders;
import org.springframework.http.HttpStatus;
import org.springframework.http.MediaType;
import org.springframework.web.reactive.function.client.ClientResponse;
import org.springframework.web.reactive.function.client.WebClient;
import reactor.core.publisher.Mono;

import java.lang.reflect.Method;
import java.lang.reflect.RecordComponent;
import java.time.LocalDateTime;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.UUID;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.mockito.ArgumentMatchers.any;
import static org.mockito.Mockito.mock;
import static org.mockito.Mockito.when;

/**
 * Hidden benchmark test: a recurring donation's monthly charges each keep their own payment.
 * The payment service is a fake that answers by method name and argument types, so a fix that
 * adds a method or a field to the payment DTO still compiles and is judged on behaviour.
 */
class RecurringDonationPaymentsHiddenTest {

    private final List<PaymentDto> rows = new ArrayList<>();
    private final UUID recurringDonation = UUID.randomUUID();
    private final UUID oneTimeDonation = UUID.randomUUID();
    private PaymentGatewayService service;
    private ISponsorshipService sponsorships;

    // --- a fake payment service, by behaviour ---------------------------------------------

    private static Object component(Object record, String name) {
        if (record == null || !record.getClass().isRecord()) return null;
        for (RecordComponent c : record.getClass().getRecordComponents()) {
            if (c.getName().equals(name)) {
                try {
                    return c.getAccessor().invoke(record);
                } catch (ReflectiveOperationException e) {
                    throw new IllegalStateException(e);
                }
            }
        }
        return null;
    }

    private PaymentDto find(UUID id) {
        return rows.stream().filter(p -> p.id().equals(id)).findFirst().orElse(null);
    }

    private void replace(PaymentDto old, PaymentDto now) {
        rows.set(rows.indexOf(old), now);
    }

    private Object answer(Method m, Object[] args) {
        String name = m.getName();
        UUID uuid = null;
        PaymentStatusEnum status = null;
        LocalDateTime date = null;
        Object dto = null;
        for (Object a : args) {
            if (a instanceof UUID u && uuid == null) uuid = u;
            else if (a instanceof PaymentStatusEnum s) status = s;
            else if (a instanceof LocalDateTime d) date = d;
            else if (a != null && a.getClass().isRecord()) dto = a;
        }
        if (dto != null && m.getReturnType().equals(PaymentDto.class) || name.startsWith("insert") || name.startsWith("create") || name.startsWith("save")) {
            UUID donationId = dto != null && component(dto, "donationId") instanceof UUID d ? d : uuid;
            PaymentStatusEnum s = dto != null && component(dto, "status") instanceof PaymentStatusEnum st ? st : status;
            LocalDateTime when = dto != null && component(dto, "date") instanceof LocalDateTime dd ? dd : (date != null ? date : LocalDateTime.now());
            PaymentDto created = new PaymentDto(UUID.randomUUID(), when, s, donationId);
            rows.add(created);
            return m.getReturnType().equals(UUID.class) ? created.id() : (m.getReturnType().equals(Void.TYPE) ? null : created);
        }
        if (name.startsWith("findByDonation")) {
            UUID d = uuid;
            return new ArrayList<>(rows.stream().filter(p -> p.donationId().equals(d)).toList());
        }
        if (name.equals("findById")) return find(uuid);
        if (name.startsWith("cancelPending")) {
            UUID d = uuid;
            for (PaymentDto p : new ArrayList<>(rows))
                if (p.donationId().equals(d) && p.status() == PaymentStatusEnum.WAITING)
                    replace(p, new PaymentDto(p.id(), p.date(), PaymentStatusEnum.CANCELED, p.donationId()));
            return null;
        }
        if (name.startsWith("update")) {
            PaymentDto p = find(uuid);
            if (p != null) {
                replace(p, new PaymentDto(p.id(), date != null ? date : p.date(), status != null ? status : p.status(), p.donationId()));
            }
            return m.getReturnType().equals(UUID.class) ? uuid : null;
        }
        throw new AssertionError("unexpected payment service call: " + name);
    }

    // --- the service under test, with every outside call faked ----------------------------

    @BeforeEach
    void setUp() {
        IPaymentService payments = mock(IPaymentService.class, invocation -> answer(invocation.getMethod(), invocation.getArguments()));
        IDonationService donations = mock(IDonationService.class);
        when(donations.findById(any())).thenAnswer(inv -> {
            UUID id = inv.getArgument(0);
            UUID sponsorship = id.equals(recurringDonation) ? UUID.randomUUID() : null;
            return new DonationDto(id, 50.0, null, PaymentGatewayEnum.PAGBANK, sponsorship,
                    PaymentMethodEnum.CREDIT_CARD, "visa", UUID.randomUUID(), LocalDateTime.now().minusMonths(3));
        });
        sponsorships = mock(ISponsorshipService.class);
        when(sponsorships.findByUserId(any())).thenReturn(List.of(new SponsorshipDto(UUID.randomUUID(), 5,
                LocalDateTime.now().minusMonths(3), true, "SUBS_1", UUID.randomUUID(), null)));
        UserService users = mock(UserService.class);
        when(users.findByDocument(any())).thenReturn(new UserDto(UUID.randomUUID(), "Ana", "ana@example.com", "12345678909", "11999999999"));

        WebClient.Builder http = WebClient.builder().exchangeFunction(request -> {
            String body = request.url().getPath().contains("subscriptions")
                    ? "{\"subscriptions\":[{\"id\":\"SUBS_1\"}]}" : "{}";
            return Mono.just(ClientResponse.create(HttpStatus.OK)
                    .header(HttpHeaders.CONTENT_TYPE, MediaType.APPLICATION_JSON_VALUE).body(body).build());
        });
        service = new PaymentGatewayService(mock(RequestCreateCheckoutPagbankMapper.class), http,
                "http://pagbank.test", "http://subscriptions.test", donations, payments, sponsorships, users,
                mock(LogPagbankService.class));
    }

    private void checkoutPayment(UUID donation, PaymentStatusEnum status, LocalDateTime date) {
        rows.add(new PaymentDto(UUID.randomUUID(), date, status, donation));
    }

    private void charge(UUID donation, String chargeId, PaymentStatusEnum status, boolean recurring) {
        Object recurringField = recurring ? Map.of("type", "SUBSEQUENT") : null;
        PaymentChargesDto charge = new PaymentChargesDto(chargeId, donation.toString(), status,
                new ResponseWebhookPaymentMethod(PaymentMethodEnum.CREDIT_CARD, new ResponseWebhookCard("visa")),
                recurringField);
        service.notifyPayment(new PaymentNotificationDto("ORDE_" + chargeId, donation, List.of(charge),
                new ResponseWebhookCustomer("Ana", "12345678909")));
    }

    private List<PaymentDto> of(UUID donation) {
        return rows.stream().filter(p -> p.donationId().equals(donation)).toList();
    }

    // --- behaviour ---------------------------------------------------------------------------

    @Test
    void theFirstChargeUpdatesTheCheckoutPaymentWithoutAddingOne() {
        checkoutPayment(recurringDonation, PaymentStatusEnum.WAITING, LocalDateTime.now().minusMinutes(5));
        charge(recurringDonation, "CHAR_1", PaymentStatusEnum.PAID, true);
        assertEquals(1, of(recurringDonation).size());
        assertEquals(PaymentStatusEnum.PAID, of(recurringDonation).getFirst().status());
    }

    @Test
    void theNextMonthsChargeIsANewPaymentAndLeavesTheEarlierOneAlone() {
        LocalDateTime august = LocalDateTime.now().minusMonths(1);
        checkoutPayment(recurringDonation, PaymentStatusEnum.PAID, august);
        charge(recurringDonation, "CHAR_2", PaymentStatusEnum.PAID, true);
        List<PaymentDto> all = of(recurringDonation);
        assertEquals(2, all.size(), "one payment per monthly charge");
        PaymentDto first = all.stream().filter(p -> p.date().equals(august)).findFirst().orElseThrow(
                () -> new AssertionError("the August payment lost its date: " + all));
        assertEquals(PaymentStatusEnum.PAID, first.status());
        assertEquals(1, all.stream().filter(p -> !p.date().equals(august) && p.status() == PaymentStatusEnum.PAID).count());
    }

    @Test
    void threeMonthsMakeThreePayments() {
        checkoutPayment(recurringDonation, PaymentStatusEnum.PAID, LocalDateTime.now().minusMonths(2));
        rows.add(new PaymentDto(UUID.randomUUID(), LocalDateTime.now().minusMonths(1), PaymentStatusEnum.PAID, recurringDonation));
        charge(recurringDonation, "CHAR_3", PaymentStatusEnum.PAID, true);
        assertEquals(3, of(recurringDonation).size());
    }

    @Test
    void aDeclinedMonthlyChargeIsRecordedWithoutTouchingThePaidOne() {
        LocalDateTime august = LocalDateTime.now().minusMonths(1);
        checkoutPayment(recurringDonation, PaymentStatusEnum.PAID, august);
        charge(recurringDonation, "CHAR_4", PaymentStatusEnum.DECLINED, true);
        List<PaymentDto> all = of(recurringDonation);
        assertEquals(2, all.size());
        assertEquals(1, all.stream().filter(p -> p.date().equals(august) && p.status() == PaymentStatusEnum.PAID).count());
        assertEquals(1, all.stream().filter(p -> p.status() == PaymentStatusEnum.DECLINED).count());
    }

    @Test
    void aRepeatedNotificationInTheSameMonthAddsNothing() {
        checkoutPayment(recurringDonation, PaymentStatusEnum.PAID, LocalDateTime.now().minusMonths(1));
        charge(recurringDonation, "CHAR_5", PaymentStatusEnum.PAID, true);
        charge(recurringDonation, "CHAR_5", PaymentStatusEnum.PAID, true);
        assertEquals(2, of(recurringDonation).size());
    }

    @Test
    void aOneTimeDonationNeverGetsASecondPayment() {
        checkoutPayment(oneTimeDonation, PaymentStatusEnum.WAITING, LocalDateTime.now().minusMinutes(5));
        charge(oneTimeDonation, "CHAR_6", PaymentStatusEnum.PAID, false);
        charge(oneTimeDonation, "CHAR_6", PaymentStatusEnum.PAID, false);
        assertEquals(1, of(oneTimeDonation).size());
        assertEquals(PaymentStatusEnum.PAID, of(oneTimeDonation).getFirst().status());
    }

    @Test
    void theSponsorshipIsStillActivatedForARecurringCharge() {
        // Unchanged behaviour: the subscription is looked up and the sponsorship updated.
        checkoutPayment(recurringDonation, PaymentStatusEnum.WAITING, LocalDateTime.now().minusMinutes(5));
        charge(recurringDonation, "CHAR_7", PaymentStatusEnum.PAID, true);
        assertEquals(1, of(recurringDonation).size());
        Mockito.verify(sponsorships).update(any());
    }
}
